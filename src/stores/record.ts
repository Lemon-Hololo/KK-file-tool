import { defineStore } from "pinia";
import type { HashIndexRecord, HashIndexRecordSummary } from "../types/record";
import { deleteHashRecord, listHashRecords, loadHashRecord, renameHashRecord } from "../services/record";

export const useRecordStore = defineStore("record", {
  state: () => ({
    list: [] as HashIndexRecordSummary[],
    selectedRecordId: localStorage.getItem("selectedRecordId") || ""
  }),
  actions: {
    async refresh() {
      this.list = await listHashRecords();
    },
    async detail(recordId: string): Promise<HashIndexRecord> {
      return loadHashRecord(recordId);
    },
    select(recordId: string) {
      this.selectedRecordId = recordId || "";
      localStorage.setItem("selectedRecordId", this.selectedRecordId);
    },
    async rename(recordId: string, newName: string) {
      await renameHashRecord(recordId, newName);
      await this.refresh();
    },
    async remove(recordId: string) {
      await deleteHashRecord(recordId);
      await this.refresh();
      if (this.selectedRecordId === recordId) this.select("");
    }
  }
});