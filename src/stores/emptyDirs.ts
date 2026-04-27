/**
 * 空文件夹清理的前端状态管理。
 *
 * 流程：preview 拿到可删除目录 → apply 删除并写库 → 暴露 `lastApplyResult`
 * 驱动"撤回本次/选中"。
 */

import { defineStore } from "pinia";
import type {
  EmptyDirApplyResponse,
  EmptyDirPreviewItem,
  EmptyDirRecordDetail,
  EmptyDirRecordSummary
} from "../types/emptyDirs";
import {
  applyEmptyDirCleanup,
  checkEmptyDirRollback,
  deleteEmptyDirRecord,
  getEmptyDirRecordDetail,
  listEmptyDirRecords,
  previewEmptyDirs,
  rollbackEmptyDirCleanup
} from "../services/emptyDirs";

export const useEmptyDirsStore = defineStore("emptyDirs", {
  state: () => ({
    previewList: [] as EmptyDirPreviewItem[],
    lastApplyResult: null as EmptyDirApplyResponse | null,
    records: [] as EmptyDirRecordSummary[],
    currentDetail: null as EmptyDirRecordDetail | null
  }),

  actions: {
    /** 拉取可删除空目录预览；顺带清空上次 apply 结果。 */
    async preview(paths: string[], includeRoots: boolean) {
      this.lastApplyResult = null;
      this.previewList = await previewEmptyDirs(paths, includeRoots);
    },

    /** 删除空文件夹并缓存可撤回记录结果。 */
    async apply(
      paths: string[],
      includeRoots: boolean,
      recordName?: string | null,
      selectedOldPaths?: string[] | null
    ) {
      const result = await applyEmptyDirCleanup(
        paths,
        includeRoots,
        recordName,
        selectedOldPaths
      );
      this.lastApplyResult = result;
      return result;
    },

    /** 刷新记录列表。 */
    async refreshRecords() {
      this.records = await listEmptyDirRecords();
    },

    /** 加载记录详情（抽屉展示用）。 */
    async loadDetail(recordId: string) {
      this.currentDetail = await getEmptyDirRecordDetail(recordId);
      return this.currentDetail;
    },

    checkRollback(recordId: string, itemIds?: number[] | null) {
      return checkEmptyDirRollback(recordId, itemIds);
    },

    rollback(recordId: string, itemIds?: number[] | null, forceIgnoreMissing = false) {
      return rollbackEmptyDirCleanup(recordId, itemIds, forceIgnoreMissing);
    },

    async remove(recordId: string) {
      await deleteEmptyDirRecord(recordId);
      await this.refreshRecords();
      if (this.currentDetail?.summary.recordId === recordId) {
        this.currentDetail = null;
      }
    },

    async removeBatch(recordIds: string[]) {
      for (const id of recordIds) {
        await deleteEmptyDirRecord(id);
      }
      await this.refreshRecords();
      if (this.currentDetail && recordIds.includes(this.currentDetail.summary.recordId)) {
        this.currentDetail = null;
      }
    }
  }
});
