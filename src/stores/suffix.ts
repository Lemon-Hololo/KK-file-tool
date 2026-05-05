/**
 * 后缀批量修改的前端状态管理。
 *
 * 流程：preview 拿到预览列表 → 用户勾选 → apply 写库 + rename →
 * 暴露 `lastApplyResult` 驱动"撤回本次/选中"。
 *
 * 记录 CRUD（list/detail/delete/rollback/...）由 [_opRecordCrud.ts](_opRecordCrud.ts)
 * 工厂统一生成；本 store 只声明业务专属的 preview / apply。
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
import { createOpRecordCrudActions } from "./_opRecordCrud";

const crud = createOpRecordCrudActions<SuffixRecordSummary, SuffixRecordDetail>({
  list: listSuffixChangeRecords,
  loadDetail: getSuffixChangeRecordDetail,
  remove: deleteSuffixChangeRecord,
  checkRollback: checkSuffixRollback,
  rollback: rollbackSuffixChange
});

export const useSuffixStore = defineStore("suffix", {
  state: () => ({
    previewList: [] as SuffixPreviewItem[],
    lastApplyResult: null as SuffixApplyResponse | null,
    records: [] as SuffixRecordSummary[],
    currentDetail: null as SuffixRecordDetail | null
  }),

  actions: {
    ...crud,

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
    }
  }
});
