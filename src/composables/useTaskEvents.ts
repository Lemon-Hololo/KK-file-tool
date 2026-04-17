import { onEvent } from "../services/tauri";
import type { TaskLogPayload, TaskProgressPayload, TaskStatus } from "../types/common";
import type { DuplicateGroup } from "../types/task";
import type { MoveReport } from "../types/moveReport";

export async function useTaskEvents(handlers: {
  onLog?: (log: TaskLogPayload) => void;
  onProgress?: (progress: TaskProgressPayload) => void;
  onPartial?: (payload: { taskId: string; groups: DuplicateGroup[]; done: boolean }) => void;
  onState?: (payload: { taskId: string; status: TaskStatus }) => void;
  onCompleted?: (payload: { taskId: string; groups: DuplicateGroup[] }) => void;
  onMoveReport?: (payload: { taskId: string; report: MoveReport; updatedGroups: DuplicateGroup[] }) => void;
}) {
  const unlisteners = await Promise.all([
    onEvent<TaskLogPayload>("task_log", (p) => handlers.onLog?.(p)),
    onEvent<TaskProgressPayload>("task_progress", (p) => handlers.onProgress?.(p)),
    onEvent<{ taskId: string; groups: DuplicateGroup[]; done: boolean }>("task_result_partial", (p) =>
      handlers.onPartial?.(p)
    ),
    onEvent<{ taskId: string; status: TaskStatus }>("task_state_changed", (p) => handlers.onState?.(p)),
    onEvent<{ taskId: string; groups: DuplicateGroup[] }>("task_completed", (p) => handlers.onCompleted?.(p)),
    onEvent<{ taskId: string; report: MoveReport; updatedGroups: DuplicateGroup[] }>("move_report_ready", (p) =>
      handlers.onMoveReport?.(p)
    )
  ]);

  return () => {
    unlisteners.forEach((off) => off());
  };
}