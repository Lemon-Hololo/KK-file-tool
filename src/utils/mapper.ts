import type { DuplicateGroup, FileEntry } from "../types/task";

function mapFile(raw: any): FileEntry {
  return {
    absPath: raw.absPath ?? raw.abs_path ?? "",
    size: raw.size ?? 0,
    mtime: raw.mtime ?? 0,
    ctime: raw.ctime ?? raw.mtime ?? 0, // 如果没有 ctime，使用 mtime 作为后备
    hash: raw.hash ?? undefined,
    selectedForMove: raw.selectedForMove ?? raw.selected_for_move ?? false,
    fromHistory: raw.fromHistory ?? raw.from_history ?? false
  };
}

export function mapGroup(raw: any): DuplicateGroup {
  return {
    groupId: raw.groupId ?? raw.group_id ?? "",
    hash: raw.hash ?? "",
    files: (raw.files ?? []).map(mapFile)
  };
}