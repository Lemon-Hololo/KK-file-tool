/**
 * 通用展示格式化工具：字节数、时间戳、apply / rollback 结果 toast。
 *
 * apply / rollback 文案抽到这里是因为同样的字符串在 OpsPanel、ModDuplicate、
 * ModVersion、RecordManagePage、ModScan 等 6+ 处重复拼接。改文案不应该跨多个文件。
 */

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/**
 * 时间戳（秒）→ `yyyy-MM-dd HH:mm:ss`。0 / 空值返回 `"-"`，便于表格里直接展示。
 */
export function formatTimestamp(timestamp: number): string {
  if (!timestamp) return "-";

  const date = new Date(timestamp * 1000);

  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");

  return `${year}-${month}-${day} ${hours}:${minutes}:${seconds}`;
}

/** apply 完成提示：`完成：成功 X，失败 Y`。 */
export function formatApplyToast(result: { success: number; failed: number }): string {
  return `完成：成功 ${result.success}，失败 ${result.failed}`;
}

/** 撤回完成提示：`撤回完成：成功 X，失败 Y，跳过缺失 Z`。 */
export function formatRollbackToast(result: {
  success: number;
  failed: number;
  skippedMissing: number;
}): string {
  return `撤回完成：成功 ${result.success}，失败 ${result.failed}，跳过缺失 ${result.skippedMissing}`;
}

/** 部分撤回完成提示：用于"撤回选中"路径，与 formatRollbackToast 仅前缀差异。 */
export function formatPartialRollbackToast(result: {
  success: number;
  failed: number;
  skippedMissing: number;
}): string {
  return `部分撤回完成：成功 ${result.success}，失败 ${result.failed}，跳过缺失 ${result.skippedMissing}`;
}
