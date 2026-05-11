/**
 * 图片相似度去重的前端状态管理。
 *
 * 与 modTools 同型：
 * - 一个长任务（扫描）通过 `runLocalLongTask` 启动，结果靠 `image_dedup_partial`
 *   事件增量推送，每 chunk 一次；
 * - 增量分组通过 `pendingGroups` Map upsert（按 groupId 去重），任务终态
 *   `done = true` 时 flush 到 `groups`；
 * - apply（删除）走 `runWithLocalTask` 同步任务模板；
 * - 记录 CRUD 由 `_opRecordCrud.ts` 工厂统一生成（含 rename）。
 *
 * 与 mod 重复检查的差异：
 * - keep 选取由后端按 `image_dedup_keep_policy` 设置完成（4 种策略），
 *   前端只需把 `files[0]` 标记为 keep，其余默认勾选删除；
 * - 每组带 `similarity` 字段，UI 上有"按相似度区间过滤"的滑块。
 */

import { defineStore } from "pinia";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  ImageDedupApplyResponse,
  ImageDedupGroup,
  ImageDedupPartialPayload,
  ImageDedupRecordDetail,
  ImageDedupRecordSummary
} from "../types/imageDedup";
import {
  applyImageDedupDelete,
  checkImageDedupRollback,
  deleteImageDedupRecord,
  getImageDedupRecordDetail,
  listImageDedupRecords,
  onImageDedupPartial,
  renameImageDedupRecord,
  rollbackImageDedup,
  startImageDedupTask
} from "../services/imageDedup";
import { stopTask } from "../services/task";
import { runLocalLongTask } from "../composables/useLocalLongTask";
import { createLocalTaskId } from "../utils/taskId";
import { upsertGroupsByKey } from "../utils/groupUpsert";
import { useRuntimeStore } from "./runtime";
import { createOpRecordCrudActionsWithRename } from "./_opRecordCrud";

interface ScanState {
  taskId: string | null;
  running: boolean;
}

interface BusyState {
  scan: boolean;
  apply: boolean;
}

let pendingGroups = new Map<string, ImageDedupGroup>();

const crud = createOpRecordCrudActionsWithRename<
  ImageDedupRecordSummary,
  ImageDedupRecordDetail
>({
  list: listImageDedupRecords as () => Promise<ImageDedupRecordSummary[]>,
  loadDetail: getImageDedupRecordDetail,
  remove: deleteImageDedupRecord,
  rename: renameImageDedupRecord,
  checkRollback: checkImageDedupRollback,
  rollback: rollbackImageDedup
});

export const useImageDedupStore = defineStore("imageDedup", {
  state: () => ({
    groups: [] as ImageDedupGroup[],
    applyResult: null as ImageDedupApplyResponse | null,
    records: [] as ImageDedupRecordSummary[],
    currentDetail: null as ImageDedupRecordDetail | null,
    scan: {
      taskId: null,
      running: false
    } as ScanState,
    busy: {
      scan: false,
      apply: false
    } as BusyState,
    _partialUnlisten: null as UnlistenFn | null
  }),

  actions: {
    ...crud,

    /** 增量合并新到的分组（按 groupId 去重）。 */
    upsertGroups(incoming: ImageDedupGroup[]) {
      this.groups = upsertGroupsByKey(this.groups, incoming);
    },

    /** 启动扫描长任务。 */
    async startScan(paths: string[]) {
      await this.initEvents();
      const taskId = await runLocalLongTask({
        prefix: "image-dedup",
        startMessage: "开始扫描相似图片",
        beforeStart: () => {
          this.applyResult = null;
          this.groups = [];
          pendingGroups = new Map();
        },
        setBusy: (v) => {
          this.busy.scan = v;
          this.scan.running = v;
          if (!v) this.scan.taskId = null;
        },
        start: (id) => startImageDedupTask(paths, id)
      });
      this.scan.taskId = taskId;
      return taskId;
    },

    /** 取消扫描；后端会发一次 done = true 的空 partial 关闭运行态。 */
    async stopScan() {
      if (!this.scan.taskId) return;
      await stopTask(this.scan.taskId);
    },

    /** 删除选中的相似图片，并把成功删除的从内存分组里剔除。 */
    async applyDelete(
      paths: string[],
      selectedFilePaths: string[],
      recordName?: string | null
    ) {
      const runtimeStore = useRuntimeStore();
      const taskId = createLocalTaskId("image-dedup-apply");
      this.busy.apply = true;
      runtimeStore.setRunningTask(taskId);
      runtimeStore.appendLocalLog(taskId, "INFO", "开始删除选中相似图片");
      try {
        const result = await applyImageDedupDelete(
          paths,
          selectedFilePaths,
          recordName,
          taskId
        );
        this.applyResult = result;
        const deleted = new Set(
          result.items
            .filter((item) => item.status === "success")
            .map((item) => item.oldPath.toLowerCase())
        );
        this.groups = this.groups
          .map((group) => ({
            ...group,
            files: group.files.filter(
              (file) => !deleted.has(file.filePath.toLowerCase())
            )
          }))
          .filter((group) => group.files.length > 1);
        runtimeStore.finishLocalTask(taskId, "Completed");
        return result;
      } catch (error) {
        runtimeStore.failLocalTask(
          taskId,
          error instanceof Error ? error.message : String(error)
        );
        throw error;
      } finally {
        this.busy.apply = false;
      }
    },

    /** 安装 partial 事件监听器；幂等。 */
    async initEvents() {
      if (this._partialUnlisten) return;
      this._partialUnlisten = await onImageDedupPartial(
        (payload: ImageDedupPartialPayload) => {
          if (this.scan.taskId && payload.taskId !== this.scan.taskId) return;
          if (payload.groups.length) {
            for (const group of payload.groups) {
              pendingGroups.set(group.groupId, withDefaultSelection(group));
            }
            // 实时刷新（图片去重每个 chunk 64 张，不会过密）。
            this.groups = upsertGroupsByKey(
              this.groups,
              Array.from(pendingGroups.values()).sort((a, b) =>
                a.groupId.localeCompare(b.groupId)
              )
            );
          }
          if (payload.done) {
            // 终态：把缓冲清空并触发一次最终 flush，让 running 状态归位。
            pendingGroups = new Map();
            this.scan.running = false;
            this.busy.scan = false;
          }
        }
      );
    }
  }
});

/** 默认勾选状态：`files[0]` = keep，其余 = 待删除。 */
function withDefaultSelection(group: ImageDedupGroup): ImageDedupGroup {
  return {
    ...group,
    files: group.files.map((file, idx) => ({
      ...file,
      selectedForDelete: idx !== 0
    }))
  };
}
