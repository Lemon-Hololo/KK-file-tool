/**
 * Pixiv 标签整理的前端状态管理。
 *
 * 一次扫描的生命周期：
 * 1. 用户点 "开始扫描"：调同步候选扫描立即建出全部 pending 行；
 * 2. 启动长任务（taskId 由前端预生成）；
 * 3. 后端 `pixiv_tag_partial` 增量到达：用 `pid → 行索引` 表 O(1) 定位，
 *    把 `this.rows[idx]` 用整个对象替换（不 in-place 写字段——in-place 在
 *    Pinia + Vue 3 嵌套 reactive 上偶发不触发 panel 级 computed 重算，
 *    见 [`moveByTag`] 与 partial 处理里同样的注释）；
 * 4. `done = true` 时把 running 标记关掉。
 *
 * 移动操作：[`moveByTag`] 调后端命令、把行的 `absPath` 替换为新路径，行保留在表中。
 * 重试：[`retry`] 调单条命令，刷新该行的 tags / status。
 *
 * 输出目录用 `useStorage` 跨会话保存；其它运行态数据死于刷新。
 */

import { defineStore } from "pinia";
import { useStorage } from "@vueuse/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  PixivImageState,
  PixivTagPartialPayload
} from "../types/pixivTag";
import {
  fetchPixivTagSingle,
  moveImageByTag,
  onPixivTagPartial,
  scanPixivImageCandidates,
  startPixivTagScan
} from "../services/pixivTag";
import { stopTask } from "../services/task";
import { useRuntimeStore } from "./runtime";

export const usePixivTagStore = defineStore("pixivTag", {
  state: () => ({
    /** 输出目录（点击 tag 单元格后图片落到这里）。跨会话持久化。 */
    outputDir: useStorage<string>("pixivTag:outputDir", ""),
    /** 当前扫描的所有候选行；按扫描顺序排列。 */
    rows: [] as PixivImageState[],
    /** 当前任务 ID；为 null 表示未启动。 */
    taskId: null as string | null,
    running: false,
    /** 已完成（成功 + 失败）的行数。 */
    completedCount: 0,
    /** 失败行数。 */
    errorCount: 0,
    /** 取消标记（运行态）。 */
    cancelled: false,
    _unlisten: null as UnlistenFn | null,
    /** pid → row index 索引；rows 重建时同步重建，避免 O(n) 扫描。 */
    _rowIndex: new Map<string, number>()
  }),

  actions: {
    /** 注册事件监听；幂等。一定要在 startScan 之前完成。 */
    async initEvents() {
      if (this._unlisten) return;
      this._unlisten = await onPixivTagPartial((payload: PixivTagPartialPayload) => {
        if (this.taskId && payload.taskId !== this.taskId) return;

        for (const item of payload.items) {
          const idx = this._rowIndex.get(item.pid);
          if (idx === undefined) continue;
          const row = this.rows[idx];
          if (!row) continue;
          // 用整个对象替换 row 而不是 in-place 改字段。
          //
          // 为什么不 in-place：原本面板里 chip 列读 row.translations 走 panel 级
          // computed 触发响应；但 in-place 写嵌套属性 `row.translations = newObj`
          // 在 Pinia + Vue 3 reactive proxy 嵌套 + 多次 partial 增量到达的组合下，
          // 翻译开关切换时 computed 偶发不重算（panel 已观察到现象 —— translations
          // 字段先一次写入空对象再写入新对象时，computed 缓存把旧 display 留住）。
          // 改成"整批字段一次性换引用"的做法跟 moveByTag 一致，强制 store.rows[idx]
          // 指针变化，依赖 store.rows 的 panel 级 computed 必然重算。
          if (item.tags) {
            this.rows[idx] = {
              ...row,
              tags: item.tags,
              // 后端没 en 译名时不发字段；这里一律落空对象，省得面板侧重复判 null。
              translations: item.translations ?? {},
              status: "ok",
              error: null
            };
          } else if (item.error) {
            this.rows[idx] = {
              ...row,
              tags: [],
              translations: {},
              status: "error",
              error: item.error
            };
          }
        }

        // 重新计算 completed / error 计数。partial 可能多次到达同一 PID（重试场景），
        // 单调累加会算错；这里用 rows 的实际 status 重新统计。
        this._recomputeCounts();

        if (payload.done) {
          this.running = false;
          const runtimeStore = useRuntimeStore();
          if (this.taskId) {
            runtimeStore.finishLocalTask(
              this.taskId,
              this.cancelled ? "Cancelled" : "Completed"
            );
          }
        }
      });
    },

    _recomputeCounts() {
      let done = 0;
      let err = 0;
      for (const r of this.rows) {
        if (r.status !== "pending") done += 1;
        if (r.status === "error") err += 1;
      }
      this.completedCount = done;
      this.errorCount = err;
    },

    /**
     * 启动扫描：
     * 1. 同步拿候选 → 立即建出 pending 行；
     * 2. 启动长任务异步拉 tag。
     */
    async startScan(paths: string[]) {
      await this.initEvents();
      this.cancelled = false;

      // 1) 同步候选扫描：失败直接抛错给调用方
      const candidates = await scanPixivImageCandidates(paths);
      const rows: PixivImageState[] = candidates.map((c) => ({
        absPath: c.absPath,
        fileName: c.fileName,
        pid: c.pid,
        status: "pending",
        tags: [],
        translations: {},
        error: null,
        movedTag: null
      }));
      this.rows = rows;
      this._rowIndex = new Map();
      rows.forEach((r, i) => this._rowIndex.set(r.pid, i));
      this.completedCount = 0;
      this.errorCount = 0;

      // 候选为空：不启动后台任务，直接结束
      if (rows.length === 0) {
        this.running = false;
        this.taskId = null;
        return;
      }

      this.running = true;

      const runtimeStore = useRuntimeStore();
      const taskId = createLocalTaskId("pixiv-tag");
      this.taskId = taskId;
      runtimeStore.setRunningTask(taskId);
      runtimeStore.appendLocalLog(
        taskId,
        "INFO",
        `识别到 ${rows.length} 张含 PID 的图片，开始拉取 tag`
      );

      // 2) 启动长任务（仅传 PID 列表，候选信息已在前端的 rows 里）
      const pids = rows.map((r) => r.pid);
      try {
        await startPixivTagScan(pids, taskId);
      } catch (e) {
        this.running = false;
        this.taskId = null;
        runtimeStore.failLocalTask(
          taskId,
          e instanceof Error ? e.message : String(e)
        );
        throw e;
      }
    },

    /** 请求停止当前任务；不会立即把 running 关掉，等 done partial。 */
    async stop() {
      if (!this.taskId) return;
      this.cancelled = true;
      await stopTask(this.taskId);
    },

    /** 单条重试。失败时把行状态改为 error 并抛错由调用方决定 toast。 */
    async retry(pid: string) {
      const idx = this._rowIndex.get(pid);
      if (idx === undefined) return;
      const row = this.rows[idx];
      if (!row) return;
      // 跟 partial 处理一致：用对象替换而不是 in-place 写嵌套字段，确保依赖
      // store.rows 的 panel 级 computed 必然重算。
      this.rows[idx] = {
        ...row,
        status: "pending",
        error: null
      };
      this._recomputeCounts();
      try {
        const res = await fetchPixivTagSingle(pid);
        const cur = this.rows[idx];
        if (!cur) return;
        this.rows[idx] = {
          ...cur,
          tags: res.tags,
          translations: res.translations ?? {},
          status: "ok",
          error: null
        };
      } catch (e) {
        const cur = this.rows[idx];
        if (cur) {
          this.rows[idx] = {
            ...cur,
            tags: [],
            translations: {},
            status: "error",
            error: e instanceof Error ? e.message : String(e)
          };
        }
        throw e;
      } finally {
        this._recomputeCounts();
      }
    },

    /**
     * 把行的图片移动到 `<outputDir>/<tag>/`，把 row.absPath 替换为新路径，
     * 并记下 `movedTag` 给 UI 显示"当前位置"。
     *
     * 行**保留**，方便用户继续往别的 tag 文件夹移动同一张图。
     *
     * 实现细节：先把单元格里的 chip 高亮立刻打上（`row.movedTag = tag`），再发 IPC；
     * IPC 回来后用**整个对象替换** `this.rows[idx]`——in-place 改字段在某些边缘情况下
     * （Pinia + Vue 3 reactive proxy 嵌套较深时）会让"已移动到"那一列的 slot 不重渲染，
     * 直接换引用最稳妥，VirtualTable 拿到新引用后那一行的所有 cell 都会重渲。
     */
    async moveByTag(pid: string, tag: string) {
      if (!this.outputDir) {
        throw new Error("请先选择输出目录");
      }
      const idx = this._rowIndex.get(pid);
      if (idx === undefined) {
        throw new Error("行不存在");
      }
      const row = this.rows[idx];
      if (!row) throw new Error("行不存在");
      const newPath = await moveImageByTag(row.absPath, this.outputDir, tag);
      // 用新对象替换行,强制 VirtualTable 行级重渲染(虚拟表只在 row.data 引用变化或
      // 显式 reactive 字段触发时才走完整 cell 更新路径,这种"先改 absPath 再改 movedTag
      // 然后等 UI 看见"的多字段批量改最稳的姿势就是直接换引用)。
      this.rows[idx] = {
        ...row,
        absPath: newPath,
        movedTag: tag
      };
    }
  }
});

function createLocalTaskId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
