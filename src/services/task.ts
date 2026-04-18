/**
 * 去重 / 运行时控制 / 移动的命令封装。对应后端 `commands::dedup`、
 * `commands::runtime`、`commands::move_file`、`commands::path`。
 */

import type { DedupConfig, NormalizePathResult } from "../types/task";
import type { MoveActionResponse, MoveSummary } from "../types/moveReport";
import { invokeCmd } from "./tauri";

/** 路径规范化：去重、去不可访问、去被父目录覆盖的子目录。 */
export function normalizeInputPaths(paths: string[]) {
  return invokeCmd<NormalizePathResult>("normalize_input_paths", { paths });
}

/** 启动一次去重任务；返回 `task_id`。 */
export function startDedupTask(paths: string[], config: DedupConfig) {
  return invokeCmd<string>("start_dedup_task", { paths, config });
}

/** 暂停指定任务。 */
export function pauseTask(taskId: string) {
  return invokeCmd<void>("pause_task", { taskId });
}

/** 恢复指定任务。 */
export function resumeTask(taskId: string) {
  return invokeCmd<void>("resume_task", { taskId });
}

/** 请求取消任务（已入队工作可能继续跑完）。 */
export function stopTask(taskId: string) {
  return invokeCmd<void>("stop_task", { taskId });
}

/** 计算移动前的汇总信息。 */
export function getMoveSummary(selectedFiles: string[], moveTargetPath?: string | null) {
  return invokeCmd<MoveSummary>("get_move_summary", {
    selectedFiles,
    moveTargetPath: moveTargetPath ?? null
  });
}

/** 执行移动，返回报告并更新内存中的重复组。 */
export function applyMoveAction(
  taskId: string,
  selectedFiles: string[],
  moveTargetPath?: string | null
) {
  return invokeCmd<MoveActionResponse>("apply_move_action", {
    taskId,
    selectedFiles,
    moveTargetPath: moveTargetPath ?? null
  });
}