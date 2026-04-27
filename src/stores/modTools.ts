/**
 * Mod 工具的前端状态管理。
 *
 * Mod 工具共用一个 store：
 * - 重命名 / 归类 / 重复删除 / 旧版本删除：preview / apply / rollback 流水线；
 * - 扫描：长任务，通过 `task_id` 关联事件，完成时由 `mod_scan_completed`
 *   事件刷新 `scan` 子状态。
 */

import { defineStore } from "pinia";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  ModDuplicateGroup,
  ModDuplicatePartialPayload,
  ModIdentityFile,
  ModOpApplyResponse,
  ModOpRecordDetail,
  ModOpRecordSummary,
  ModOrganizePreviewItem,
  ModRenamePreviewItem,
  ModScanCompletedPayload,
  ModScanMatch,
  ModVersionGroup,
  ModVersionPartialPayload
} from "../types/modTools";
import {
  applyModDuplicateDelete,
  applyModModifyVersion,
  applyModOrganize,
  applyModRename,
  applyModVersionDelete,
  checkModOpRollback,
  deleteModOpRecord,
  getModOpRecordDetail,
  listModOpRecords,
  onModDuplicatePartial,
  onModScanCompleted,
  onModVersionPartial,
  previewModOrganize,
  previewModRename,
  renameModOpRecord,
  rollbackModOp,
  startModDuplicateTask,
  startModScanTask,
  startModVersionTask
} from "../services/modTools";
import { stopTask } from "../services/task";
import { useConfigStore } from "./config";
import { useRuntimeStore } from "./runtime";

interface ScanState {
  taskId: string | null;
  running: boolean;
  keyword: string;
  matches: ModScanMatch[];
  totalScanned: number;
  totalErrors: number;
  cancelled: boolean;
}

interface BusyState {
  rename: boolean;
  organize: boolean;
  duplicates: boolean;
  versions: boolean;
  modify: boolean;
}

interface GroupCheckState {
  taskId: string | null;
  running: boolean;
}

let pendingDuplicateGroups = new Map<string, ModDuplicateGroup>();
let pendingVersionGroups = new Map<string, ModVersionGroup>();

export const useModToolsStore = defineStore("modTools", {
  state: () => ({
    renamePreview: [] as ModRenamePreviewItem[],
    renameApplyResult: null as ModOpApplyResponse | null,
    organizePreview: [] as ModOrganizePreviewItem[],
    organizeApplyResult: null as ModOpApplyResponse | null,
    duplicateGroups: [] as ModDuplicateGroup[],
    duplicateApplyResult: null as ModOpApplyResponse | null,
    versionGroups: [] as ModVersionGroup[],
    versionApplyResult: null as ModOpApplyResponse | null,
    records: [] as ModOpRecordSummary[],
    currentDetail: null as ModOpRecordDetail | null,
    duplicateCheck: {
      taskId: null,
      running: false
    } as GroupCheckState,
    versionCheck: {
      taskId: null,
      running: false
    } as GroupCheckState,
    scan: {
      taskId: null,
      running: false,
      keyword: "",
      matches: [],
      totalScanned: 0,
      totalErrors: 0,
      cancelled: false
    } as ScanState,
    busy: {
      rename: false,
      organize: false,
      duplicates: false,
      versions: false,
      modify: false
    } as BusyState,
    _scanUnlisten: null as UnlistenFn | null,
    _duplicateUnlisten: null as UnlistenFn | null,
    _versionUnlisten: null as UnlistenFn | null
  }),

  actions: {
    upsertDuplicateGroups(incoming: ModDuplicateGroup[]) {
      const map = new Map(this.duplicateGroups.map((group) => [group.groupId, group]));
      for (const group of incoming) map.set(group.groupId, group);
      this.duplicateGroups = Array.from(map.values()).sort((a, b) => a.groupId.localeCompare(b.groupId));
    },

    upsertVersionGroups(incoming: ModVersionGroup[]) {
      const map = new Map(this.versionGroups.map((group) => [group.groupId, group]));
      for (const group of incoming) map.set(group.groupId, group);
      this.versionGroups = Array.from(map.values()).sort((a, b) => a.groupId.localeCompare(b.groupId));
    },

    async previewRename(paths: string[]) {
      await runWithLocalTask(this.busy, "rename", async (taskId) => {
        this.renameApplyResult = null;
        this.renamePreview = await previewModRename(paths, taskId);
      });
    },

    async applyRename(
      paths: string[],
      recordName?: string | null,
      selectedOldPaths?: string[] | null
    ) {
      return await runWithLocalTask(this.busy, "rename", async (taskId) => {
        const result = await applyModRename(paths, recordName, selectedOldPaths, taskId);
        this.renameApplyResult = result;
        return result;
      });
    },

    async previewOrganize(paths: string[]) {
      await runWithLocalTask(this.busy, "organize", async (taskId) => {
        this.organizeApplyResult = null;
        this.organizePreview = await previewModOrganize(paths, taskId);
      });
    },

    async applyOrganize(
      paths: string[],
      recordName?: string | null,
      selectedOldPaths?: string[] | null
    ) {
      return await runWithLocalTask(this.busy, "organize", async (taskId) => {
        const result = await applyModOrganize(paths, recordName, selectedOldPaths, taskId);
        this.organizeApplyResult = result;
        return result;
      });
    },

    /** 按 `guid + author + version` 查找重复 MOD；默认保留策略跟随配置中心。 */
    async previewDuplicates(paths: string[]) {
      await this.initDuplicateEvents();
      const runtimeStore = useRuntimeStore();
      const taskId = createLocalTaskId("mod-duplicates");
      this.duplicateApplyResult = null;
      this.duplicateGroups = [];
      pendingDuplicateGroups = new Map();
      this.duplicateCheck.taskId = taskId;
      this.duplicateCheck.running = true;
      this.busy.duplicates = true;
      runtimeStore.setRunningTask(taskId);
      runtimeStore.appendLocalLog(taskId, "INFO", "开始检查重复 MOD");
      try {
        await startModDuplicateTask(paths, taskId);
      } catch (error) {
        this.busy.duplicates = false;
        this.duplicateCheck.running = false;
        this.duplicateCheck.taskId = null;
        runtimeStore.failLocalTask(
          taskId,
          error instanceof Error ? error.message : String(error)
        );
        throw error;
      }
    },

    /** 删除重复 MOD 中选中的文件，并缓存可撤回记录结果。 */
    async applyDuplicateDelete(
      paths: string[],
      selectedFilePaths: string[],
      recordName?: string | null
    ) {
      return await runWithLocalTask(this.busy, "duplicates", async (taskId) => {
        const result = await applyModDuplicateDelete(
          paths,
          selectedFilePaths,
          recordName,
          taskId
        );
        this.duplicateApplyResult = result;
        const deleted = new Set(
          result.items
            .filter((item) => item.status === "success")
            .map((item) => item.oldPath.toLowerCase())
        );
        this.duplicateGroups = this.duplicateGroups
          .map((group) => ({
            ...group,
            files: group.files.filter((file) => !deleted.has(file.filePath.toLowerCase()))
          }))
          .filter((group) => group.files.length > 1);
        return result;
      });
    },

    /** 按 `guid + author` 查找不同版本 MOD；默认保留策略跟随配置中心。 */
    async previewVersions(paths: string[]) {
      await this.initVersionEvents();
      const runtimeStore = useRuntimeStore();
      const taskId = createLocalTaskId("mod-versions");
      this.versionApplyResult = null;
      this.versionGroups = [];
      pendingVersionGroups = new Map();
      this.versionCheck.taskId = taskId;
      this.versionCheck.running = true;
      this.busy.versions = true;
      runtimeStore.setRunningTask(taskId);
      runtimeStore.appendLocalLog(taskId, "INFO", "开始检查不同版本 MOD");
      try {
        await startModVersionTask(paths, taskId);
      } catch (error) {
        this.busy.versions = false;
        this.versionCheck.running = false;
        this.versionCheck.taskId = null;
        runtimeStore.failLocalTask(
          taskId,
          error instanceof Error ? error.message : String(error)
        );
        throw error;
      }
    },

    /** 删除不同版本 MOD 中选中的文件，并缓存可撤回记录结果。 */
    async applyVersionDelete(
      paths: string[],
      selectedFilePaths: string[],
      recordName?: string | null
    ) {
      return await runWithLocalTask(this.busy, "versions", async (taskId) => {
        const result = await applyModVersionDelete(
          paths,
          selectedFilePaths,
          recordName,
          taskId
        );
        this.versionApplyResult = result;
        const deleted = new Set(
          result.items
            .filter((item) => item.status === "success")
            .map((item) => item.oldPath.toLowerCase())
        );
        this.versionGroups = this.versionGroups
          .map((group) => ({
            ...group,
            files: group.files.filter((file) => !deleted.has(file.filePath.toLowerCase()))
          }))
          .filter((group) => new Set(group.files.map((file) => file.version)).size > 1);
        return result;
      });
    },

    async refreshRecords(kind?: string | null) {
      this.records = await listModOpRecords(kind);
    },

    async loadDetail(recordId: string) {
      this.currentDetail = await getModOpRecordDetail(recordId);
      return this.currentDetail;
    },

    checkRollback(recordId: string, itemIds?: number[] | null) {
      return checkModOpRollback(recordId, itemIds);
    },

    rollback(recordId: string, itemIds?: number[] | null, forceIgnoreMissing = false) {
      return rollbackModOp(recordId, itemIds, forceIgnoreMissing);
    },

    async remove(recordId: string) {
      await deleteModOpRecord(recordId);
      await this.refreshRecords();
      if (this.currentDetail?.summary.recordId === recordId) {
        this.currentDetail = null;
      }
    },

    async removeBatch(recordIds: string[]) {
      for (const id of recordIds) {
        await deleteModOpRecord(id);
      }
      await this.refreshRecords();
      if (this.currentDetail && recordIds.includes(this.currentDetail.summary.recordId)) {
        this.currentDetail = null;
      }
    },

    async rename(recordId: string, newName: string) {
      await renameModOpRecord(recordId, newName);
      await this.refreshRecords();
      if (this.currentDetail?.summary.recordId === recordId) {
        this.currentDetail.summary.recordName = newName;
      }
    },

    async initScanEvents() {
      if (this._scanUnlisten) return;
      this._scanUnlisten = await onModScanCompleted((p: ModScanCompletedPayload) => {
        if (this.scan.taskId && p.taskId !== this.scan.taskId) return;
        this.scan.running = false;
        this.scan.matches = p.matches;
        this.scan.totalScanned = p.totalScanned;
        this.scan.totalErrors = p.totalErrors;
        this.scan.cancelled = p.cancelled;
      });
    },

    async initDuplicateEvents() {
      if (this._duplicateUnlisten) return;
      this._duplicateUnlisten = await onModDuplicatePartial((payload: ModDuplicatePartialPayload) => {
        if (this.duplicateCheck.taskId && payload.taskId !== this.duplicateCheck.taskId) return;
        if (payload.groups.length) {
          const configStore = useConfigStore();
          collectDuplicateGroups(
            payload.groups,
            configStore.settings.keepPolicy
          );
        }
        if (payload.done) {
          this.duplicateGroups = flushDuplicateGroups();
          this.duplicateCheck.running = false;
          this.busy.duplicates = false;
        }
      });
    },

    async initVersionEvents() {
      if (this._versionUnlisten) return;
      this._versionUnlisten = await onModVersionPartial((payload: ModVersionPartialPayload) => {
        if (this.versionCheck.taskId && payload.taskId !== this.versionCheck.taskId) return;
        if (payload.groups.length) {
          const configStore = useConfigStore();
          collectVersionGroups(
            payload.groups,
            configStore.settings.keepPolicy
          );
        }
        if (payload.done) {
          this.versionGroups = flushVersionGroups();
          this.versionCheck.running = false;
          this.busy.versions = false;
        }
      });
    },

    async startScan(paths: string[], keyword: string) {
      await this.initScanEvents();
      const runtimeStore = useRuntimeStore();
      const taskId = createLocalTaskId("mod-scan");
      this.scan.keyword = keyword;
      this.scan.matches = [];
      this.scan.totalScanned = 0;
      this.scan.totalErrors = 0;
      this.scan.cancelled = false;
      this.scan.running = true;
      runtimeStore.setRunningTask(taskId);
      runtimeStore.appendLocalLog(taskId, "INFO", `开始扫描版本限制，关键字: ${keyword}`);
      try {
        this.scan.taskId = await startModScanTask(paths, keyword, taskId);
        return this.scan.taskId;
      } catch (error) {
        this.scan.running = false;
        this.scan.taskId = null;
        runtimeStore.failLocalTask(
          taskId,
          error instanceof Error ? error.message : String(error)
        );
        throw error;
      }
    },

    async stopScan() {
      if (!this.scan.taskId) return;
      await stopTask(this.scan.taskId);
    },

    async stopDuplicateCheck() {
      if (!this.duplicateCheck.taskId) return;
      await stopTask(this.duplicateCheck.taskId);
    },

    async stopVersionCheck() {
      if (!this.versionCheck.taskId) return;
      await stopTask(this.versionCheck.taskId);
    },

    /**
     * 对扫描结果中的选中项应用"移除版本限制"修改。
     *
     * 成功后返回的 `ModOpApplyResponse` 也会写入 `renameApplyResult` 的位置
     * 并不合适，因此仅作为返回值交给调用者决定 UI 反馈。
     */
    async applyModifyVersion(
      paths: string[],
      keyword: string,
      selectedFilePaths: string[],
      recordName?: string | null
    ) {
      return await runWithLocalTask(this.busy, "modify", async (taskId) => {
        return await applyModModifyVersion(
          paths,
          keyword,
          selectedFilePaths,
          recordName,
          taskId
        );
      });
    }
  }
});

type BusyKey = keyof BusyState;

function createLocalTaskId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

async function runWithLocalTask<T>(
  busyState: BusyState,
  key: BusyKey,
  runner: (taskId: string) => Promise<T>
): Promise<T> {
  const runtimeStore = useRuntimeStore();
  const taskId = createLocalTaskId(`mod-${key}`);
  busyState[key] = true;
  runtimeStore.setRunningTask(taskId);
  runtimeStore.appendLocalLog(taskId, "INFO", taskStartMessage(key));
  try {
    const result = await runner(taskId);
    runtimeStore.finishLocalTask(taskId, "Completed");
    return result;
  } catch (error) {
    runtimeStore.failLocalTask(
      taskId,
      error instanceof Error ? error.message : String(error)
    );
    throw error;
  } finally {
    busyState[key] = false;
  }
}

function taskStartMessage(key: BusyKey): string {
  if (key === "rename") return "开始预览或执行 Mod 重命名";
  if (key === "organize") return "开始预览或执行 Mod 归类";
  if (key === "duplicates") return "开始检查重复 MOD";
  if (key === "versions") return "开始检查不同版本 MOD";
  return "开始处理 Mod 版本限制";
}

function collectDuplicateGroups(
  groups: ModDuplicateGroup[],
  keepPolicy: "newest" | "oldest"
) {
  for (const group of withDuplicateKeepPolicy(groups, keepPolicy)) {
    pendingDuplicateGroups.set(group.groupId, group);
  }
}

function flushDuplicateGroups() {
  const groups = Array.from(pendingDuplicateGroups.values()).sort((a, b) =>
    a.groupId.localeCompare(b.groupId)
  );
  pendingDuplicateGroups = new Map();
  return groups;
}

function withDuplicateKeepPolicy<T extends { files: ModIdentityFile[] }>(
  groups: T[],
  keepPolicy: "newest" | "oldest"
) {
  return groups.map((group) => {
    const sorted = [...group.files].sort((a, b) => b.mtime - a.mtime);
    const keep =
      keepPolicy === "newest" ? sorted[0] : sorted[sorted.length - 1];
    return {
      ...group,
      files: group.files.map((file) => ({
        ...file,
        selectedForDelete: keep ? file.filePath !== keep.filePath : false
      }))
    };
  });
}

function collectVersionGroups(
  groups: ModVersionGroup[],
  keepPolicy: "newest" | "oldest"
) {
  for (const group of withVersionKeepPolicy(groups, keepPolicy)) {
    pendingVersionGroups.set(group.groupId, group);
  }
}

function flushVersionGroups() {
  const groups = Array.from(pendingVersionGroups.values()).sort((a, b) =>
    a.groupId.localeCompare(b.groupId)
  );
  pendingVersionGroups = new Map();
  return groups;
}

function withVersionKeepPolicy<T extends ModVersionGroup>(
  groups: T[],
  keepPolicy: "newest" | "oldest"
) {
  return groups.map((group) => {
    const keepVersion =
      keepPolicy === "newest"
        ? group.latestVersion
        : [...group.files]
            .sort(compareVersionFilesAscending)
            .at(0)
            ?.version ?? group.latestVersion;

    return {
      ...group,
      files: group.files.map((file) => ({
        ...file,
        selectedForDelete: file.version !== keepVersion
      }))
    };
  });
}

function compareVersionFilesAscending(a: ModIdentityFile, b: ModIdentityFile) {
  const byVersion = compareVersionText(a.version, b.version);
  if (byVersion !== 0) return byVersion;
  if (a.mtime !== b.mtime) return a.mtime - b.mtime;
  return a.filePath.localeCompare(b.filePath);
}

function compareVersionText(a: string, b: string) {
  const aParts = splitVersionParts(a);
  const bParts = splitVersionParts(b);
  const maxLen = Math.max(aParts.length, bParts.length);

  for (let i = 0; i < maxLen; i += 1) {
    const aPart = aParts[i] ?? "0";
    const bPart = bParts[i] ?? "0";
    const byPart = compareVersionPart(aPart, bPart);
    if (byPart !== 0) return byPart;
  }

  return a.localeCompare(b);
}

function splitVersionParts(version: string) {
  return version
    .split(/[^0-9a-zA-Z]+/)
    .filter(Boolean)
    .map((part) => part.toLowerCase());
}

function compareVersionPart(a: string, b: string) {
  const aIsNum = /^\d+$/.test(a);
  const bIsNum = /^\d+$/.test(b);
  const aNum = aIsNum ? Number.parseInt(a, 10) : 0;
  const bNum = bIsNum ? Number.parseInt(b, 10) : 0;

  if (aIsNum && bIsNum) return aNum - bNum;
  if (aIsNum && !bIsNum) return 1;
  if (!aIsNum && bIsNum) return -1;
  return a.localeCompare(b);
}
