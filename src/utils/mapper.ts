/**
 * 去重结果的前后端字段映射。
 *
 * 后端通过 `#[serde(rename_all = "camelCase")]` 输出驼峰；这里的 mapper
 * 只是一层薄透传，保留函数形式便于将来引入额外的字段校验。
 */

import type { DuplicateGroup, FileEntry } from "../types/task";

function mapFile(raw: any): FileEntry {
  return {
    absPath: raw.absPath ?? "",
    size: raw.size ?? 0,
    mtime: raw.mtime ?? 0,
    ctime: raw.ctime ?? raw.mtime ?? 0,
    hash: raw.hash ?? undefined,
    selectedForMove: raw.selectedForMove ?? false,
    fromHistory: raw.fromHistory ?? false
  };
}

/** 把事件 payload 或命令返回里的 group 映射为前端模型。 */
export function mapGroup(raw: any): DuplicateGroup {
  return {
    groupId: raw.groupId ?? "",
    hash: raw.hash ?? "",
    files: (raw.files ?? []).map(mapFile)
  };
}
