import { invoke } from "@tauri-apps/api/core";
import type {
  DesktopGoalCommandResult,
  DesktopGoalStatus,
  DesktopGoalStep,
} from "./desktopTypes";

export function goalStatus(): Promise<DesktopGoalStatus> {
  if (!isTauriRuntime()) {
    if (isPreviewGoalFixture()) {
      return Promise.resolve(previewGoalStatus("Preview daily desktop goal"));
    }
    return Promise.resolve({
      goal_id: null,
      objective: null,
      status: null,
      turn_count: null,
      max_turns: null,
      last_decision: null,
      last_closeout: null,
      last_proof: null,
      last_blocker: null,
      step_count: 0,
      steps: [],
    });
  }
  return invoke("goal_status");
}

export function goalStart(objective: string): Promise<DesktopGoalCommandResult> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      status: {
        goal_id: "preview-goal",
        objective,
        status: "Active",
        turn_count: 0,
        max_turns: 10,
        last_decision: null,
        last_closeout: null,
        last_proof: null,
        last_blocker: null,
        step_count: 0,
        steps: [],
      },
      next_prompt: `Goal: ${objective}\n\nWork toward this objective. Take the smallest useful step first.`,
    });
  }
  return invoke("goal_start", { objective });
}

export function goalPause(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return Promise.resolve(false);
  }
  return invoke("goal_pause");
}

export function goalResume(): Promise<DesktopGoalCommandResult> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      status: {
        goal_id: "preview-goal",
        objective: "Preview goal",
        status: "Active",
        turn_count: 0,
        max_turns: 10,
        last_decision: null,
        last_closeout: null,
        last_proof: null,
        last_blocker: null,
        step_count: 0,
        steps: [],
      },
      next_prompt: "Continue working toward the active goal.",
    });
  }
  return invoke("goal_resume");
}

export function goalClear(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return Promise.resolve(false);
  }
  return invoke("goal_clear");
}

export function goalEdit(objective: string): Promise<DesktopGoalStatus> {
  if (!isTauriRuntime()) {
    return Promise.resolve({
      goal_id: "preview-goal",
      objective,
      status: "Active",
      turn_count: 0,
      max_turns: 10,
      last_decision: null,
      last_closeout: null,
      last_proof: null,
      last_blocker: null,
      step_count: 0,
      steps: [],
    });
  }
  return invoke("goal_edit", { objective });
}

export function goalLog(): Promise<DesktopGoalStep[]> {
  if (!isTauriRuntime()) {
    return Promise.resolve([]);
  }
  return invoke("goal_log");
}

function isTauriRuntime() {
  if (typeof window === "undefined" || !window.__TAURI_INTERNALS__) {
    return false;
  }

  const internals = window.__TAURI_INTERNALS__ as {
    invoke?: unknown;
    transformCallback?: unknown;
  };
  return typeof internals.invoke === "function" && typeof internals.transformCallback === "function";
}

function isPreviewGoalFixture() {
  if (typeof window === "undefined") {
    return false;
  }
  return new URLSearchParams(window.location.search).get("previewFixture") === "goal";
}

function previewGoalStatus(objective: string): DesktopGoalStatus {
  return {
    goal_id: "preview-goal",
    objective,
    status: "Active",
    turn_count: 3,
    max_turns: 10,
    last_decision: "continue",
    last_closeout: null,
    last_proof: "desktop smoke",
    last_blocker: null,
    step_count: 2,
    steps: [],
  };
}
