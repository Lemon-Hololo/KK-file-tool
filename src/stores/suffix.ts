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
    async preview(paths: string[], targetSuffix: string) {
      this.previewList = await previewSuffixChange(paths, targetSuffix);
    },

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

    async refreshRecords() {
      this.records = await listSuffixChangeRecords();
    },

    async loadDetail(recordId: string) {
      this.currentDetail = await getSuffixChangeRecordDetail(recordId);
      return this.currentDetail;
    },

    async checkRollback(recordId: string, itemIds?: number[] | null) {
      return checkSuffixRollback(recordId, itemIds);
    },

    async rollback(recordId: string, itemIds?: number[] | null, forceIgnoreMissing = false) {
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
