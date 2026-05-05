/**
 * 空文件夹清理的前端状态管理。
 *
 * 流程：preview 拿到可删除目录 → apply 删除并写库 → 暴露 `lastApplyResult`
 * 驱动"撤回本次/选中"。
 *
 * 记录 CRUD 由 [_opRecordCrud.ts](_opRecordCrud.ts) 工厂统一生成。
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
import { createOpRecordCrudActions } from "./_opRecordCrud";

const crud = createOpRecordCrudActions<EmptyDirRecordSummary, EmptyDirRecordDetail>({
  list: listEmptyDirRecords,
  loadDetail: getEmptyDirRecordDetail,
  remove: deleteEmptyDirRecord,
  checkRollback: checkEmptyDirRollback,
  rollback: rollbackEmptyDirCleanup
});

export const useEmptyDirsStore = defineStore("emptyDirs", {
  state: () => ({
    previewList: [] as EmptyDirPreviewItem[],
    lastApplyResult: null as EmptyDirApplyResponse | null,
    records: [] as EmptyDirRecordSummary[],
    currentDetail: null as EmptyDirRecordDetail | null
  }),

  actions: {
    ...crud,

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
    }
  }
});
