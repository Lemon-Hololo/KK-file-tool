/**
 * "记录管理"页面的共用列定义。
 *
 * 列定义集中在这里是为了：
 * 1. 多个 tab 复用相同的"记录名/创建时间/成功数/总数/回滚状态"列；
 * 2. 列宽 / formatter 调整时不必跨多个 .vue 文件搜索；
 * 3. 详情抽屉的列也常被多处共用（哈希记录除外，它在自有 Drawer 里）。
 */

import type { VirtualColumn } from "../types/virtualTable";
import { formatTimestamp } from "../utils/format";

/** 记录管理页 Mod 操作类型展示文案。 */
export function formatModKind(kind: string): string {
  if (kind === "rename") return "Mod 重命名";
  if (kind === "organize") return "文件夹归类";
  if (kind === "modify") return "移除版本限制";
  if (kind === "duplicate_delete") return "删除重复 MOD";
  if (kind === "version_delete") return "删除旧版本 MOD";
  return kind;
}

/** 空文件夹清理记录的操作类型展示文案。 */
export function formatEmptyDirKind(kind: string): string {
  if (kind === "delete") return "删除空文件夹";
  return kind;
}

/** 哈希记录列表列。 */
export const hashListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 240, ellipsis: true, resizable: true },
  {
    key: "createdAt",
    label: "创建时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "entryCount", label: "条目数", width: 90 },
  { key: "__actions", label: "操作", width: 260, slotName: "hashActions" }
];

/** 后缀修改记录列表列。 */
export const suffixListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  { key: "targetSuffix", label: "目标后缀", width: 100 },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 }
];

/** 后缀修改记录详情列。 */
export const suffixDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "旧路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "新路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "修改", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];

/** 空文件夹清理记录列表列。 */
export const emptyDirsListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  {
    key: "kind",
    label: "类型",
    width: 120,
    formatter: (_row, v: string) => formatEmptyDirKind(v)
  },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 }
];

/** 空文件夹清理记录详情列。 */
export const emptyDirsDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "目录路径", minWidth: 320, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "删除", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "applyError", label: "删除错误", minWidth: 180, ellipsis: true, resizable: true },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];

/** Mod 操作记录列表列。 */
export const modListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  {
    key: "kind",
    label: "类型",
    width: 110,
    formatter: (_row, v: string) => formatModKind(v)
  },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 }
];

/** Mod 操作记录详情列。 */
export const modDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "旧路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "新路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "执行", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "applyError", label: "执行错误", minWidth: 180, ellipsis: true, resizable: true },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];

/** 图片相似度去重记录的操作类型展示文案。 */
export function formatImageDedupKind(kind: string): string {
  if (kind === "similarity_delete") return "按相似度删除";
  return kind;
}

/** 图片去重记录列表列。 */
export const imageDedupListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  {
    key: "kind",
    label: "类型",
    width: 130,
    formatter: (_row, v: string) => formatImageDedupKind(v)
  },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 }
];

/** 图片去重记录详情列。 */
export const imageDedupDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "原图路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "备份路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "删除", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "applyError", label: "删除错误", minWidth: 180, ellipsis: true, resizable: true },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];
