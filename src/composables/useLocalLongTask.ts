/**
 * 启动一条"前端预生成 task_id 的长任务"的统一封装。
 *
 * 长任务模式（Mod 重复 / 不同版本 / 扫描 / Pixiv 标签）：
 * 1. 前端预生成 task_id（避免事件早于监听器到达造成丢失）；
 * 2. runtime store 标记 running task 并写一条开始日志；
 * 3. 调 service 启动后端任务；
 * 4. 失败时把 busy / running 状态归位 + 写一条 ERROR 日志，并把错误向上抛
 *    给调用方做 toast。
 *
 * 与 modTools.ts 内部的 `runWithLocalTask` 不同：本 hook 不在 await 后写 finish/fail
 * 状态，因为长任务的终态由后端事件（`mod_*_partial done = true` / `pixiv_tag_partial`）
 * 在 store 的事件监听里处理；这里仅负责"启动失败"的兜底。
 */

import { useRuntimeStore } from "../stores/runtime";
import { createLocalTaskId } from "../utils/taskId";

export interface RunLocalLongTaskOptions {
  /** task_id 前缀，会拼上随机 UUID。 */
  prefix: string;
  /** 启动时写入 runtime 日志的 INFO 文案。 */
  startMessage: string;
  /** 启动前同步重置业务侧状态（清 preview / 缓存等）。 */
  beforeStart?: () => void;
  /** 切换 busy / running 标志的回调；接收 true 表示进入运行态。 */
  setBusy: (busy: boolean) => void;
  /** 调 service 启动后端长任务。 */
  start: (taskId: string) => Promise<unknown>;
}

/**
 * 启动一条预生成 task_id 的长任务，返回该 task_id 供调用方关联事件。
 *
 * 启动失败时：调用方拿到的 Promise 会 reject 原错误，已经清掉 busy；
 * 启动成功时：调用方拿到 task_id，busy 仍为 true，等后端事件来打回 false。
 */
export async function runLocalLongTask(opts: RunLocalLongTaskOptions): Promise<string> {
  const runtimeStore = useRuntimeStore();
  const taskId = createLocalTaskId(opts.prefix);

  opts.beforeStart?.();
  opts.setBusy(true);
  runtimeStore.setRunningTask(taskId);
  runtimeStore.appendLocalLog(taskId, "INFO", opts.startMessage);

  try {
    await opts.start(taskId);
    return taskId;
  } catch (error) {
    opts.setBusy(false);
    runtimeStore.failLocalTask(
      taskId,
      error instanceof Error ? error.message : String(error)
    );
    throw error;
  }
}
