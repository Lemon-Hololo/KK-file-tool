/**
 * 后缀批量修改的前端状态管理。
 *
 * 流程：preview 拿到预览列表 → 用户勾选 → apply 写库 + rename →
 * 暴露 `lastApplyResult` 驱动"撤回本次/选中"。
 */

import { defineStore } from "pinia";
import type {
  SuffixApplyResponse,
  SuffixPreviewItem,
  SuffixRecordDetail,
  SuffixRecordSummary
} from "../types/suffix";
import {
  applySuffixChange,
  checkSuffixRollback,
  deleteSuffixChangeRecord,
  getSuffixChangeRecordDetail,
  listSuffixChangeRecords,
  previewSuffixChange,
  rollbackSuffixChange
} from "../services/suffix";

export const useSuffixStore = defineStore("suffix", {
  state: () => ({
    previewList: [] as SuffixPreviewItem[],
    lastApplyResult: null as SuffixApplyResponse | null,
    records: [] as SuffixRecordSummary[],
    currentDetail: null as SuffixRecordDetail | null
  }),

  actions: {
    /** 拉取最新预览；顺带清空上次 apply 结果避免旧数据挡住预览渲染。 */
    async preview(paths: string[], targetSuffix: string) {
      this.lastApplyResult = null;
      this.previewList = await previewSuffixChange(paths, targetSuffix);
    },

    /** 应用修改并缓存结果。 */
    async apply(
      paths: string[],
      targetSuffix: string,
      recordName?: string | null,
      selectedOldPaths?: string[] | null
    ) {
      const result = await applySuffixChange(paths, targetSuffix, recordName, selectedOldPaths);
      this.lastApplyResult = result;
      return result;
    },

    /** 刷新记录列表。 */
    async refreshRecords() {
      this.records = await listSuffixChangeRecords();
    },

    /** 加载记录详情（抽屉展示用）。 */
    async loadDetail(recordId: string) {
      this.currentDetail = await getSuffixChangeRecordDetail(recordId);
      return this.currentDetail;
    },

    checkRollback(recordId: string, itemIds?: number[] | null) {
      return checkSuffixRollback(recordId, itemIds);
    },

    rollback(recordId: string, itemIds?: number[] | null, forceIgnoreMissing = false) {
      return rollbackSuffixChange(recordId, itemIds, forceIgnoreMissing);
    },

    async remove(recordId: string) {
      await deleteSuffixChangeRecord(recordId);
      await this.refreshRecords();
      if (this.currentDetail?.summary.recordId === recordId) {
        this.currentDetail = null;
      }
    },

    async removeBatch(recordIds: string[]) {
      for (const id of recordIds) {
        await deleteSuffixChangeRecord(id);
      }
      await this.refreshRecords();
      if (this.currentDetail && recordIds.includes(this.currentDetail.summary.recordId)) {
        this.currentDetail = null;
      }
    }
  }
});
