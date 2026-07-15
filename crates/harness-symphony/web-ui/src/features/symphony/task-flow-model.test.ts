import { describe, expect, it } from "vitest";
import { deriveForkedTaskFlow } from "./task-flow-model";
import type { TaskFlow, TaskFlowStepId } from "./types";

const prStepIds: TaskFlowStepId[] = ["start", "agent", "e2e", "validation", "pr", "review", "sync", "done"];

function flow(prStatus: string, current: TaskFlowStepId = "review"): TaskFlow {
  const stepIds = prStatus === "not_applicable"
    ? prStepIds.filter((id) => id !== "pr")
    : prStepIds;
  const currentIndex = stepIds.indexOf(current);
  return {
    story_id: "US-098",
    title: "Forked task flow",
    state: "waiting",
    current_step: current,
    message: "Ready for review.",
    pr_status: prStatus,
    steps: stepIds.map((id, index) => ({
      id,
      state: index < currentIndex ? "complete" : index === currentIndex ? "current" : "pending"
    })),
    recovery_action: null
  };
}

describe("deriveForkedTaskFlow", () => {
  it("keeps both review lanes neutral while the path is undecided", () => {
    const result = deriveForkedTaskFlow(flow("missing", "agent"));

    expect(result.prLane.status).toBe("candidate");
    expect(result.localLane.status).toBe("candidate");
    expect(result.prLane.steps.map((step) => step.state)).toEqual(["pending", "pending"]);
    expect(result.localLane.steps.map((step) => step.state)).toEqual(["pending"]);
  });

  it("dims both lanes when the flow passed the join without a recorded path", () => {
    const result = deriveForkedTaskFlow(flow("missing", "done"));

    expect(result.prLane.status).toBe("not-taken");
    expect(result.localLane.status).toBe("not-taken");
    expect(result.prLane.steps.every((step) => step.state === null)).toBe(true);
    expect(result.localLane.steps[0].state).toBeNull();
  });

  it("maps canonical PR states onto the PR lane and dims local review", () => {
    const result = deriveForkedTaskFlow(flow("created"));

    expect(result.prLane.status).toBe("taken");
    expect(result.prLane.steps.map((step) => step.state)).toEqual(["complete", "current"]);
    expect(result.localLane.status).toBe("not-taken");
    expect(result.localLane.steps[0].state).toBeNull();
  });

  it("maps local-review states onto the local lane and dims the PR lane", () => {
    const result = deriveForkedTaskFlow(flow("not_applicable"));

    expect(result.localLane.status).toBe("taken");
    expect(result.localLane.steps[0].state).toBe("current");
    expect(result.prLane.status).toBe("not-taken");
    expect(result.prLane.steps.every((step) => step.state === null)).toBe(true);
  });
});
