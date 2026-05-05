/**
 * 危险 / 缺失提示弹窗的统一封装。
 *
 * 撤回前的"X 个文件不存在，仅撤回存在文件，继续？"提示在 OpsPanel、
 * RecordManagePage（suffix / emptyDirs / mod 三处）、ModDuplicate / ModVersion
 * 的 rollbackLast 都各自手写一份。把它收敛到这里，文案可单点修改。
 */

import { ElMessageBox } from "element-plus";

/**
 * 撤回前缺失提示。`count = 0` 时直接 resolve（不弹框）。
 *
 * `prefix` 用于"选中项中"前缀（OpsPanel 撤回选中走它）；不传时沿用通用文案。
 * `noun` 用于自定义"文件" / "备份文件" / "目录" 的称谓 —— 空文件夹清理是
 * 路径占用而非文件缺失，传 `noun: "路径"` 改文案。
 */
export async function confirmMissingPaths(
  count: number,
  options: {
    prefix?: string;
    noun?: string;
    title?: string;
    /** 占用语义 = true 时文案改成"路径被非目录文件占用"；默认 false（缺失语义）。 */
    occupied?: boolean;
  } = {}
): Promise<void> {
  if (!count) return;
  const { prefix = "", noun = "文件", title = "缺失提示", occupied = false } = options;
  const reason = occupied ? `${noun}被非目录文件占用` : `${noun}不存在`;
  const action = occupied ? "仅恢复可创建目录" : `仅撤回存在${noun}`;
  await ElMessageBox.confirm(
    `${prefix}有 ${count} 个${reason}，${action}，继续？`,
    title,
    { type: "warning" }
  );
}
