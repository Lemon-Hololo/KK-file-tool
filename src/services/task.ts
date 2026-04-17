import type { DedupConfig, NormalizePathResult } from "../types/task";
import type { MoveActionResponse, MoveSummary } from "../types/moveReport";
import { invokeCmd } from "./tauri";

export function normalizeInputPaths(paths: string[]) {
  return invokeCmd<NormalizePathResult>("normalize_input_paths", { paths });
}

export function startDedupTask(paths: string[], config: DedupConfig) {
  return invokeCmd<string>("start_dedup_task", { paths, config });
}

export function pauseTask(taskId: string) {
  return invokeCmd<void>("pause_task", { taskId });
}

export function resumeTask(taskId: string) {
  return invokeCmd<void>("resume_task", { taskId });
}

export function stopTask(taskId: string) {
  return invokeCmd<void>("stop_task", { taskId });
}

export function getMoveSummary(selectedFiles: string[], moveTargetPath?: string | null) {
  return invokeCmd<MoveSummary>("get_move_summary", { selectedFiles, moveTargetPath: moveTargetPath ?? null });
}

export function applyMoveAction(taskId: string, selectedFiles: string[], moveTargetPath?: string | null) {
  return invokeCmd<MoveActionResponse>("apply_move_action", {
    taskId,
    selectedFiles,
    moveTargetPath: moveTargetPath ?? null
  });
}