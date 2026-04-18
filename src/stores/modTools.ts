/**
 * Mod 工具的前端状态管理。
 *
 * 三个功能共用一个 store：
 * - 重命名 / 归类：与 suffix 完全同构的 preview / apply / rollback 流水线；
 * - 扫描：长任务，通过 `task_id` 关联事件，完成时由 `mod_scan_completed`
 *   事件刷新 `scan` 子状态。
 */

import { defineStore } from "pinia";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  ModOpApplyResponse,
  ModOpRecordDetail,
  ModOpRecordSummary,
  ModOrganizePreviewItem,
  ModRenamePreviewItem,
  ModScanCompletedPayload,
  ModScanMatch
} from "../types/modTools";
import {
  applyModOrganize,
  applyModRename,
  checkModOpRollback,
  deleteModOpRecord,
  getModOpRecordDetail,
  listModOpRecords,
  onModScanCompleted,
  previewModOrganize,
  previewModRename,
  renameModOpRecord,
  rollbackModOp,
  startModScanTask
} from "../services/modTools";
import { stopTask } from "../services/task";

interface ScanState {
  taskId: string | null;
  running: boolean;
  keyword: string;
  matches: ModScanMatch[];
  totalScanned: number;
  totalErrors: number;
  cancelled: boolean;
}

export const useModToolsStore = defineStore("modTools", {
  state: () => ({
    renamePreview: [] as ModRenamePreviewItem[],
    renameApplyResult: null as ModOpApplyResponse | null,
    organizePreview: [] as ModOrganizePreviewItem[],
    organizeApplyResult: null as ModOpApplyResponse | null,
    records: [] as ModOpRecordSummary[],
    currentDetail: null as ModOpRecordDetail | null,
    scan: {
      taskId: null,
      running: false,
      keyword: "",
      matches: [],
      totalScanned: 0,
      totalErrors: 0,
      cancelled: false
    } as ScanState,
    _scanUnlisten: null as UnlistenFn | null
  }),

  actions: {
    async previewRename(paths: string[]) {
      this.renameApplyResult = null;
      this.renamePreview = await previewModRename(paths);
    },

    async applyRename(
      paths: string[],
      recordName?: string | null,
      selectedOldPaths?: string[] | null
    ) {
      const result = await applyModRename(paths, recordName, selectedOldPaths);
      this.renameApplyResult = result;
      return result;
    },

    async previewOrganize(paths: string[]) {
      this.organizeApplyResult = null;
      this.organizePreview = await previewModOrganize(paths);
    },

    async applyOrganize(
      paths: string[],
      recordName?: string | null,
      selectedOldPaths?: string[] | null
    ) {
      const result = await applyModOrganize(paths, recordName, selectedOldPaths);
      this.organizeApplyResult = result;
      return result;
    },

    async refreshRecords(kind?: "rename" | "organize" | null) {
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

    async startScan(paths: string[], keyword: string) {
      await this.initScanEvents();
      this.scan.keyword = keyword;
      this.scan.matches = [];
      this.scan.totalScanned = 0;
      this.scan.totalErrors = 0;
      this.scan.cancelled = false;
      this.scan.running = true;
      this.scan.taskId = await startModScanTask(paths, keyword);
      return this.scan.taskId;
    },

    async stopScan() {
      if (!this.scan.taskId) return;
      await stopTask(this.scan.taskId);
    }
  }
});
