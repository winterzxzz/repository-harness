import { describe, expect, it } from "vitest";
import { deriveRunAgentInfo } from "./run-info-model";
import type { NormalizedRunEvent } from "./types";

function event(message: string, kind = "lifecycle", stage = "agent"): NormalizedRunEvent {
  return { sequence: 1, timestamp: "2026-07-15T00:00:00Z", agent: "opencode", kind, stage, message };
}

describe("deriveRunAgentInfo", () => {
  it("extracts adapter and -m model from the opencode start event", () => {
    const info = deriveRunAgentInfo([
      event("agent process started: opencode run --auto -m opencode/deepseek-v4-flash-free")
    ]);
    expect(info.adapter).toBe("opencode");
    expect(info.model).toBe("opencode/deepseek-v4-flash-free");
    expect(info.command).toBe("opencode run --auto -m opencode/deepseek-v4-flash-free");
  });

  it("extracts codex -c model= overrides", () => {
    const info = deriveRunAgentInfo([
      event("agent process started: codex app-server -c approval_policy=never -c model=gpt-5.3-codex")
    ]);
    expect(info.adapter).toBe("codex");
    expect(info.model).toBe("gpt-5.3-codex");
  });

  it("returns nulls when no start event with a command exists", () => {
    expect(deriveRunAgentInfo([event("agent process started")])).toEqual({
      executor: "opencode",
      adapter: null,
      model: null,
      command: null
    });
    expect(deriveRunAgentInfo([])).toEqual({ executor: null, adapter: null, model: null, command: null });
  });

  it("reports the executor name from event attribution", () => {
    const winterEvent = { ...event("streamed line", "output"), agent: "Winter2" };
    expect(deriveRunAgentInfo([winterEvent]).executor).toBe("Winter2");
  });

  it("ignores output events that merely mention the prefix", () => {
    const info = deriveRunAgentInfo([
      event("agent process started: fake -m nope", "output"),
      event("agent process started: codex app-server")
    ]);
    expect(info.adapter).toBe("codex");
    expect(info.model).toBeNull();
  });
});
