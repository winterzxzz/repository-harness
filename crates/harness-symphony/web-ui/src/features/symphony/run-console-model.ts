import type { RunEvent } from "./types";

export const MAX_RETAINED_RUN_EVENTS = 1_000;
export const MAX_TRANSCRIPT_CHARACTERS = 200_000;

export type ConsoleCommand = {
  kind: "command";
  id: string;
  command: string;
  output: string;
  exitCode: number | null;
  failed: boolean;
  timestamp?: string;
};

export type ConsoleMessage = {
  kind: "message";
  id: string;
  source: string;
  text: string;
  timestamp?: string;
};

export type ConsoleMilestone = {
  kind: "milestone";
  id: string;
  label: string;
  text: string;
  timestamp?: string;
};

export type ConsoleBlock = ConsoleCommand | ConsoleMessage | ConsoleMilestone;

type TranscriptOptions = {
  maxCharacters?: number;
  agentName?: string;
};

type EventEnvelope = {
  payload: unknown;
  timestamp?: string;
  agent?: string;
};

export function retainRunEvents(current: RunEvent[], incoming: RunEvent[]): RunEvent[] {
  return [...current, ...incoming].slice(-MAX_RETAINED_RUN_EVENTS);
}

export function buildConsoleTranscript(events: RunEvent[], options: TranscriptOptions = {}): ConsoleBlock[] {
  const blocks: ConsoleBlock[] = [];
  const commands = new Map<string, ConsoleCommand>();
  const agentDeltas = new Map<string, string>();

  events.forEach((rawEvent, index) => {
    const envelope = unwrapEvent(rawEvent);
    const event = envelope.payload;
    const method = getString(event, ["method"]);
    const params = getValue(event, ["params"]);
    const item = getValue(params, ["item"]);
    const itemType = getString(item, ["type"]) ?? getString(params, ["type"]);

    if (isReasoningEvent(method, itemType)) {
      return;
    }

    if (method === "item/started" && isCommandType(itemType)) {
      const id = itemId(item, params, index);
      addCommand(blocks, commands, {
        kind: "command",
        id,
        command: commandFrom(item) ?? "Command started",
        output: "",
        exitCode: null,
        failed: false,
        timestamp: envelope.timestamp ?? timestampFrom(event)
      });
      return;
    }

    if (isCommandOutputMethod(method)) {
      const id = itemId(item, params, index);
      const command = commands.get(id) ?? addCommand(blocks, commands, {
        kind: "command",
        id,
        command: commandFrom(item) ?? "Command output",
        output: "",
        exitCode: null,
        failed: false,
        timestamp: envelope.timestamp ?? timestampFrom(event)
      });
      command.output += stripAnsi(textFrom(params) ?? "");
      return;
    }

    if (method === "item/agentMessage/delta") {
      const id = itemId(item, params, index);
      agentDeltas.set(id, (agentDeltas.get(id) ?? "") + (textFrom(params) ?? ""));
      return;
    }

    if (method === "item/completed" && isCommandType(itemType)) {
      const id = itemId(item, params, index);
      const command = commands.get(id) ?? addCommand(blocks, commands, {
        kind: "command",
        id,
        command: commandFrom(item) ?? "Command completed",
        output: "",
        exitCode: null,
        failed: false,
        timestamp: envelope.timestamp ?? timestampFrom(event)
      });
      const exitCode = getNumber(item, ["exitCode"]) ?? getNumber(params, ["exitCode"]);
      command.exitCode = exitCode ?? null;
      command.failed = exitCode !== undefined && exitCode !== 0;
      if (!command.output) {
        command.output = stripAnsi(
          getString(item, ["aggregatedOutput"]) ?? getString(item, ["output"]) ?? ""
        );
      }
      return;
    }

    if (method === "item/completed" && isAgentMessageType(itemType)) {
      const id = itemId(item, params, index);
      const text = stripAnsi(textFrom(item) ?? agentDeltas.get(id) ?? "").trim();
      if (text) {
        blocks.push({
          kind: "message",
          id,
          source: envelope.agent ?? options.agentName ?? "Agent",
          text,
          timestamp: envelope.timestamp ?? timestampFrom(event)
        });
      }
      return;
    }

    const normalized = normalizedMilestone(rawEvent, index);
    if (normalized) {
      blocks.push(normalized);
    }
  });

  return capTranscript(blocks, options.maxCharacters ?? MAX_TRANSCRIPT_CHARACTERS);
}

export function stripAnsi(value: string): string {
  return value.replace(
    /[\u001B\u009B](?:\][^\u0007]*(?:\u0007|\u001B\\)|[[\]()#;?]*(?:(?:(?:[a-zA-Z\d]*(?:;[-a-zA-Z\d/#&.:=?%@~_]+)*)?\u0007)|(?:(?:\d{1,4}(?:[;:]\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~])))/g,
    ""
  );
}

function unwrapEvent(event: RunEvent): EventEnvelope {
  const normalizedMessage = getString(event, ["message"]);
  const timestamp = getString(event, ["timestamp"]);
  const agent = getString(event, ["agent"]);
  if (normalizedMessage?.trimStart().startsWith("{")) {
    try {
      const payload: unknown = JSON.parse(normalizedMessage);
      if (getString(payload, ["method"]) || getValue(payload, ["id"]) !== undefined) {
        return { payload, timestamp, agent };
      }
    } catch {
      // A normalized event can contain ordinary text that begins with a brace.
    }
  }
  return { payload: event, timestamp, agent };
}

function normalizedMilestone(event: RunEvent, index: number): ConsoleMilestone | null {
  const kind = getString(event, ["kind"]);
  const stage = getString(event, ["stage"]);
  const message = getString(event, ["message"]);
  if (!kind || !stage || !message || isReasoningEvent(undefined, stage)) {
    return null;
  }
  return {
    kind: "milestone",
    id: String(getNumber(event, ["sequence"]) ?? `event-${index}`),
    label: stage,
    text: stripAnsi(message),
    timestamp: getString(event, ["timestamp"])
  };
}

function addCommand(
  blocks: ConsoleBlock[],
  commands: Map<string, ConsoleCommand>,
  command: ConsoleCommand
): ConsoleCommand {
  blocks.push(command);
  commands.set(command.id, command);
  return command;
}

function capTranscript(blocks: ConsoleBlock[], maxCharacters: number): ConsoleBlock[] {
  if (maxCharacters <= 0) {
    return [];
  }
  const retained: ConsoleBlock[] = [];
  let remaining = maxCharacters;
  for (let index = blocks.length - 1; index >= 0 && remaining > 0; index -= 1) {
    const block = blocks[index];
    const size = blockSize(block);
    if (size <= remaining) {
      retained.unshift(block);
      remaining -= size;
      continue;
    }
    if (retained.length === 0) {
      retained.unshift(trimBlock(block, remaining));
    }
    break;
  }
  return retained;
}

function blockSize(block: ConsoleBlock): number {
  if (block.kind === "command") {
    return block.command.length + block.output.length;
  }
  return block.text.length;
}

function trimBlock(block: ConsoleBlock, limit: number): ConsoleBlock {
  if (block.kind === "command") {
    if (block.command.length >= limit) {
      return { ...block, command: block.command.slice(-limit), output: "" };
    }
    const outputLimit = Math.max(0, limit - block.command.length);
    return { ...block, output: outputLimit > 0 ? block.output.slice(-outputLimit) : "" };
  }
  return { ...block, text: block.text.slice(-limit) };
}

function itemId(item: unknown, params: unknown, index: number): string {
  return getString(item, ["id"]) ?? getString(params, ["itemId"]) ?? `item-${index}`;
}

function commandFrom(item: unknown): string | null {
  const command = getValue(item, ["command"]);
  if (typeof command === "string") {
    return stripAnsi(command);
  }
  if (Array.isArray(command)) {
    return command.filter((part): part is string => typeof part === "string").join(" ");
  }
  return null;
}

function isCommandOutputMethod(method: string | undefined): boolean {
  return method === "item/commandExecution/outputDelta" || method === "item/outputDelta";
}

function isReasoningEvent(method: string | undefined, itemType: string | undefined): boolean {
  return Boolean(method?.toLowerCase().includes("reasoning") || itemType?.toLowerCase().includes("reasoning"));
}

function isCommandType(itemType: string | undefined): boolean {
  return itemType?.replaceAll("_", "").toLowerCase() === "commandexecution";
}

function isAgentMessageType(itemType: string | undefined): boolean {
  return itemType?.replaceAll("_", "").toLowerCase() === "agentmessage";
}

function textFrom(value: unknown): string | null {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    const text = value.map(textFrom).filter((part): part is string => Boolean(part)).join("");
    return text || null;
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

function timestampFrom(event: unknown): string | undefined {
  return getString(event, ["params", "timestamp"]) ?? getString(event, ["params", "item", "createdAt"]);
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
