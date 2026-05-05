/**
 * 本地任务 ID 生成。
 *
 * 多个长任务（Mod 扫描 / 重复检查 / 不同版本 / Pixiv 标签）由前端预生成 task_id
 * 后传给后端，避免事件早于监听器到达造成丢失。原本各 store 都内联了同样的实现。
 */

/** 生成 `<prefix>-<uuid>` 形式的任务 ID；环境无 randomUUID 时回退到时间戳 + 随机串。 */
export function createLocalTaskId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
