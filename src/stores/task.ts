/**
 * 去重任务结果的前端状态管理。
 *
 * 订阅 `task_result_partial` / `task_completed` / `move_report_ready`
 * 三类事件，把后端增量推送汇入 `resultGroups`。
 */

import { defineStore } from "pinia";
import { onEvent } from "../services/tauri";
import { startDedupTask, applyMoveAction } from "../services/task";
import { mapGroup } from "../utils/mapper";
import type { DedupConfig, DuplicateGroup } from "../types/task";
import type { MoveActionResponse, MoveReport } from "../types/moveReport";
import { useRuntimeStore } from "./runtime";
import { useRecordStore } from "./record";

export const useTaskStore = defineStore("task", {
  state: () => ({
    resultGroups: [] as DuplicateGroup[],
    latestMoveReport: null as MoveReport | null,
    _inited: false
  }),

  getters: {
    selectedMoveCount(state): number {
      return state.resultGroups.reduce(
        (acc, g) => acc + g.files.filter((f) => f.selectedForMove).length,
        0
      );
    },

    selectedMoveBytes(state): number {
      return state.resultGroups.reduce((acc, g) => {
        return acc + g.files.filter((f) => f.selectedForMove).reduce((a, b) => a + (b.size || 0), 0);
      }, 0);
    }
  },

  actions: {
    upsertGroups(incoming: DuplicateGroup[]) {
      const map = new Map(this.resultGroups.map((g) => [g.groupId, g]));
      for (const g of incoming) map.set(g.groupId, g);
      this.resultGroups = Array.from(map.values()).sort((a, b) => a.groupId.localeCompare(b.groupId));
    },

    async initEvents() {
      if (this._inited) return;
      this._inited = true;

      const runtimeStore = useRuntimeStore();

      await onEvent<any>("task_result_partial", (payload) => {
        if (runtimeStore.taskId && payload?.taskId !== runtimeStore.taskId) return;
        const groups = (payload?.groups || []).map((x: any) => mapGroup(x));
        this.upsertGroups(groups);
      });

      await onEvent<any>("task_completed", (payload) => {
        if (runtimeStore.taskId && payload?.taskId !== runtimeStore.taskId) return;
        this.resultGroups = (payload?.groups || []).map((x: any) => mapGroup(x));
        // 任务完成后刷新历史记录列表
        const recordStore = useRecordStore();
        recordStore.refresh();
      });

      await onEvent<any>("move_report_ready", (payload) => {
        if (runtimeStore.taskId && payload?.taskId !== runtimeStore.taskId) return;
        this.latestMoveReport = payload.report as MoveReport;
        this.resultGroups = (payload.updatedGroups || []).map((x: any) => mapGroup(x));
        // 移动完成后刷新历史记录列表
        const recordStore = useRecordStore();
        recordStore.refresh();
      });
    },

    async start(paths: string[], config: DedupConfig) {
      const runtimeStore = useRuntimeStore();

      this.resultGroups = [];
      this.latestMoveReport = null;

      const taskId = await startDedupTask(paths, config);
      runtimeStore.setRunningTask(taskId);
    },

    async moveSelected(selectedFiles: string[], moveTargetPath?: string | null) {
      const runtimeStore = useRuntimeStore();
      if (!runtimeStore.taskId || !selectedFiles.length) return null;

      const resp = await applyMoveAction(runtimeStore.taskId, selectedFiles, moveTargetPath || null);
      this.latestMoveReport = resp.report;
      this.resultGroups = (resp.updatedGroups || []).map((x: any) => mapGroup(x));
      return resp as MoveActionResponse;
    }
  }
});
