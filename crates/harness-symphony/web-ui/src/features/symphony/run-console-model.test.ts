import { describe, expect, test } from "vitest";
import {
  MAX_RETAINED_RUN_EVENTS,
  buildConsoleTranscript,
  retainRunEvents,
  stripAnsi
} from "./run-console-model";

describe("buildConsoleTranscript", () => {
  test("groups streamed output beneath its command and marks non-zero exits", () => {
    const transcript = buildConsoleTranscript([
      {
        method: "item/started",
        params: { item: { id: "cmd-1", type: "commandExecution", command: "npm test" } }
      },
      {
        method: "item/commandExecution/outputDelta",
        params: { itemId: "cmd-1", delta: "first line\n" }
      },
      {
        method: "item/commandExecution/outputDelta",
        params: { itemId: "cmd-1", delta: "\u001b[31mfailed\u001b[0m\n" }
      },
      {
        method: "item/completed",
        params: {
          item: { id: "cmd-1", type: "commandExecution", command: "npm test", exitCode: 1 }
        }
      }
    ]);

    expect(transcript).toEqual([
      expect.objectContaining({
        kind: "command",
        id: "cmd-1",
        command: "npm test",
        output: "first line\nfailed\n",
        exitCode: 1,
        failed: true
      })
    ]);
  });

  test("decodes app-server events wrapped by normalized run event envelopes", () => {
    const transcript = buildConsoleTranscript([
      {
        sequence: 1,
        timestamp: "2026-07-14T10:00:00Z",
        agent: "codex",
        kind: "message",
        stage: "agent",
        message: JSON.stringify({
          method: "item/started",
          params: { item: { id: "cmd-2", type: "commandExecution", command: "cargo test" } }
        })
      },
      {
        sequence: 2,
        timestamp: "2026-07-14T10:00:01Z",
        agent: "codex",
        kind: "message",
        stage: "agent",
        message: JSON.stringify({
          method: "item/commandExecution/outputDelta",
          params: { itemId: "cmd-2", delta: "ok\n" }
        })
      }
    ]);

    expect(transcript).toEqual([
      expect.objectContaining({ kind: "command", command: "cargo test", output: "ok\n" })
    ]);
  });

  test("keeps agent completions and normalized milestones but filters reasoning noise", () => {
    const transcript = buildConsoleTranscript([
      {
        method: "item/reasoning/summaryTextDelta",
        params: { itemId: "reason-1", delta: "private chain of thought" }
      },
      {
        method: "item/completed",
        params: { item: { id: "reason-1", type: "reasoning", text: "hidden reasoning" } }
      },
      {
        method: "item/completed",
        params: { item: { id: "message-1", type: "agentMessage", text: "Implementation complete." } }
      },
      {
        sequence: 4,
        timestamp: "2026-07-14T10:00:00Z",
        agent: "codex",
        kind: "progress",
        stage: "validation",
        message: "Tests passed"
      }
    ]);

    expect(transcript).toEqual([
      expect.objectContaining({ kind: "message", text: "Implementation complete." }),
      expect.objectContaining({ kind: "milestone", label: "validation", text: "Tests passed" })
    ]);
  });

  test("caps transcript characters while preserving the newest content", () => {
    const transcript = buildConsoleTranscript(
      [
        { method: "item/completed", params: { item: { id: "old", type: "agentMessage", text: "old message" } } },
        { method: "item/completed", params: { item: { id: "new", type: "agentMessage", text: "newest message" } } }
      ],
      { maxCharacters: 14 }
    );

    expect(transcript).toEqual([
      expect.objectContaining({ kind: "message", text: "newest message" })
    ]);
  });

  test("strictly caps a single oversized command block", () => {
    const transcript = buildConsoleTranscript(
      [
        {
          method: "item/completed",
          params: {
            item: {
              id: "large-command",
              type: "commandExecution",
              command: "very-long-command",
              aggregatedOutput: "very-long-output",
              exitCode: 0
            }
          }
        }
      ],
      { maxCharacters: 8 }
    );

    expect(transcript).toEqual([
      expect.objectContaining({ kind: "command", command: "-command", output: "" })
    ]);
  });
});

test("strips ansi sequences from array-form commands", () => {
  const transcript = buildConsoleTranscript([
    {
      method: "item/started",
      params: {
        item: {
          id: "cmd-1",
          type: "commandExecution",
          command: ["npm", "[32mtest[0m"]
        }
      }
    }
  ]);

  expect(transcript).toEqual([
    expect.objectContaining({ kind: "command", id: "cmd-1", command: "npm test" })
  ]);
});

test("stripAnsi removes terminal control sequences", () => {
  expect(stripAnsi("\u001b[1;32mPASS\u001b[0m\r\n")).toBe("PASS\r\n");
});

test("retainRunEvents bounds polling state to the newest events", () => {
  const events = Array.from({ length: MAX_RETAINED_RUN_EVENTS + 2 }, (_, sequence) => ({ sequence }));

  const retained = retainRunEvents([], events);

  expect(retained).toHaveLength(MAX_RETAINED_RUN_EVENTS);
  expect(retained[0]).toEqual({ sequence: 2 });
  expect(retained.at(-1)).toEqual({ sequence: MAX_RETAINED_RUN_EVENTS + 1 });
});
