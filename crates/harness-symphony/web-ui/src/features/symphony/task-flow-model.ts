import type { TaskFlow, TaskFlowStepId, TaskFlowStepState } from "./types";

export type ForkLaneStatus = "candidate" | "taken" | "not-taken";
export type ForkStep = { id: TaskFlowStepId; state: TaskFlowStepState | null };
export type ForkLane = { status: ForkLaneStatus; steps: ForkStep[] };

export type ForkedTaskFlow = {
  head: ForkStep[];
  prLane: ForkLane;
  localLane: ForkLane;
  tail: ForkStep[];
};

const headIds: TaskFlowStepId[] = ["start", "agent", "validation"];
const tailIds: TaskFlowStepId[] = ["sync", "done"];

export function deriveForkedTaskFlow(flow: TaskFlow): ForkedTaskFlow {
  const states = new Map(flow.steps.map((step) => [step.id, step.state]));
  const selected = flow.pr_status === "not_applicable" ? "local" : flow.pr_status === "missing" ? null : "pr";
  const syncState = states.get("sync");
  const passedJoin = syncState === "complete" || syncState === "current";
  const laneStatus = (lane: "pr" | "local"): ForkLaneStatus =>
    selected === null ? (passedJoin ? "not-taken" : "candidate") : selected === lane ? "taken" : "not-taken";
  const laneSteps = (lane: "pr" | "local", ids: TaskFlowStepId[]): ForkLane => {
    const status = laneStatus(lane);
    return {
      status,
      steps: ids.map((id) => ({
        id,
        state: status === "taken" ? states.get(id) ?? "pending" : status === "candidate" ? "pending" : null
      }))
    };
  };
  const sharedSteps = (ids: TaskFlowStepId[]) => ids.map((id) => ({ id, state: states.get(id) ?? "pending" }));

  return {
    head: sharedSteps(headIds),
    prLane: laneSteps("pr", ["pr", "review"]),
    localLane: laneSteps("local", ["review"]),
    tail: sharedSteps(tailIds)
  };
}
