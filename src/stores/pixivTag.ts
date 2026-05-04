/**
 * Pixiv 标签整理的前端状态管理。
 *
 * 一次扫描的生命周期：
 * 1. 用户点 "开始扫描"：调同步候选扫描立即建出全部 pending 行；
 * 2. 启动长任务（taskId 由前端预生成）；
 * 3. 后端 `pixiv_tag_partial` 增量到达：先进入 `_pendingItems` 缓冲区，按
 *    `pixivPartialFlushIntervalMs` 配置决定立刻 flush（=0）还是按间隔节流；
 * 4. flush 时按 `_pidIndex` 把每个 PID 的结果**应用到所有同 PID 行**——
 *    很多 Pixiv 作品都有 `_p0..._pN` 多张图，文件名共用 PID，候选扫描后会
 *    出现多行同 PID。**这里必须遍历全部索引**，否则只更新第一行，其他保持
 *    pending（视觉上的"还是 ..."）；
 * 5. `done = true` 时把 running 标记关掉。
 *
 * 写入策略：每次更新**用整个对象替换** `this.rows[idx]`，不 in-place 改字段。
 * Pinia + Vue 3 reactive proxy 嵌套对象在 in-place 多次写入下偶发不触发依赖
 * （已踩过两次：moveByTag 与 partial 的 row.translations 写入），引用换掉是
 * 已知工作的最稳模式。
 *
 * 行级操作（`moveByTag` / 单行重试 / 译名覆盖）按 `absPath` 定位 —— PID 不唯一，
 * absPath 才是行的稳定主键。`_pathIndex` 维护 absPath → 行索引；`moveByTag`
 * 完成后会重写它（absPath 变了）。
 *
 * 输出目录用 `useStorage` 跨会话保存；其它运行态数据死于刷新。
 */

import { defineStore } from "pinia";
import { useStorage } from "@vueuse/core";
import type { UnlistenFn } from "@tauri-apps/api/event";

import type {
  PixivImageState,
  PixivTagPartialItem,
  PixivTagPartialPayload,
  PixivTranslationOverride
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
import { useConfigStore } from "./config";

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

    /**
     * `pid → row index 列表`。同一个 PID 可能有多行（一张作品的 p0..pN
     * 文件名共用 PID）；partial 到达时必须把 tags / translations 应用到
     * **所有索引**，否则同 PID 多张图里只有一张能跳到 ok 状态、能被点击
     * 移动，其他保持 pending 永久看不到 chip。
     *
     * 用单一索引 `Map<string, number>` 是这次重构前的写法，正是这个 bug 根因。
     */
    _pidIndex: new Map<string, number[]>(),

    /**
     * `absPath → row index`。行级操作（移动 / 单行重试 / 译名覆盖）按
     * absPath 定位，因为同 PID 多行时 PID 已经不唯一。`moveByTag` 完成
     * 后被移动的那一行 absPath 变了，需要在这里同步更新。
     */
    _pathIndex: new Map<string, number>(),

    /**
     * Partial 缓冲区：`pid → 最新的 partial item`。同一 PID 的多次 partial
     * （重试场景）会被**最后一条覆盖**，flush 时只看最新值——这是合理的，
     * 因为重试后的结果就是该 PID 当前的真值。
     */
    _pendingItems: new Map<string, PixivTagPartialItem>(),
    /** flush 节流定时器；非 null 表示有待触发的 flush。 */
    _flushTimer: null as ReturnType<typeof setTimeout> | null
  }),

  actions: {
    /** 注册事件监听；幂等。一定要在 startScan 之前完成。 */
    async initEvents() {
      if (this._unlisten) return;
      this._unlisten = await onPixivTagPartial((payload: PixivTagPartialPayload) => {
        if (this.taskId && payload.taskId !== this.taskId) return;

        // 1) 把每条 partial item 落进缓冲区（按 PID 去重）
        for (const item of payload.items) {
          this._pendingItems.set(item.pid, item);
        }

        // 2) 按"刷新间隔"决定立刻 flush 还是节流
        const interval = this._getFlushInterval();
        if (interval <= 0 || payload.done) {
          // done 终态必须立刻 flush，否则用户看到的统计永远停在节流前的最后一版。
          this._cancelFlushTimer();
          this._commitPending();
        } else if (this._flushTimer == null) {
          // 已有 timer 在排队就不再开新的，等它触发；下次 partial 到时该批
          // 也会被合并进 pending Map。
          this._flushTimer = setTimeout(() => {
            this._flushTimer = null;
            this._commitPending();
          }, interval);
        }

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

    /**
     * 把 `_pendingItems` 的所有条目应用到对应的 rows，然后清空缓冲区。
     *
     * 关键：每个 PID 找到的不是单一 idx 而是 `number[]`，整组同 PID 行一起更新。
     * 写入用 `this.rows[idx] = {...row, ...}` —— 对象替换确保依赖了 store.rows
     * 的下游 watchEffect / computed 重算（in-place 写嵌套字段在 Pinia 嵌套
     * reactive 上偶发不触发，是这次重构前 chip 不刷新的根因之一）。
     */
    _commitPending() {
      if (this._pendingItems.size === 0) return;
      for (const [pid, item] of this._pendingItems) {
        const indices = this._pidIndex.get(pid);
        if (!indices) continue;
        for (const idx of indices) {
          const row = this.rows[idx];
          if (!row) continue;
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
      }
      this._pendingItems.clear();
      this._recomputeCounts();
    },

    /** 关闭 pending 节流定时器（如果有），不触发 commit。 */
    _cancelFlushTimer() {
      if (this._flushTimer != null) {
        clearTimeout(this._flushTimer);
        this._flushTimer = null;
      }
    },

    /** 读当前的刷新间隔（ms）；从 config 读，0–10000 之间裁剪。 */
    _getFlushInterval(): number {
      const configStore = useConfigStore();
      const v = configStore.settings.pixivPartialFlushIntervalMs;
      if (typeof v !== "number" || !Number.isFinite(v) || v <= 0) return 0;
      return Math.min(10000, Math.floor(v));
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
      this._cancelFlushTimer();
      this._pendingItems.clear();

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
        movedTag: null,
        useTranslationOverride: "global"
      }));
      this.rows = rows;
      this._rebuildIndices();
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

      // 2) 启动长任务（仅传 PID 列表，候选信息已在前端的 rows 里）。
      // **去重传 PID**：同 PID 多张图共享同一拉取结果，没必要让后端拉 N 次同一个 ID。
      const uniquePids = Array.from(new Set(rows.map((r) => r.pid)));
      try {
        await startPixivTagScan(uniquePids, taskId);
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

    /** 重建 pid / path 两份索引；rows 整体替换后必须调用。 */
    _rebuildIndices() {
      const pidMap = new Map<string, number[]>();
      const pathMap = new Map<string, number>();
      this.rows.forEach((r, i) => {
        const arr = pidMap.get(r.pid);
        if (arr) arr.push(i);
        else pidMap.set(r.pid, [i]);
        pathMap.set(r.absPath, i);
      });
      this._pidIndex = pidMap;
      this._pathIndex = pathMap;
    },

    /** 请求停止当前任务；不会立即把 running 关掉，等 done partial。 */
    async stop() {
      if (!this.taskId) return;
      this.cancelled = true;
      await stopTask(this.taskId);
    },

    /**
     * 单条 PID 重试（按行）。同 PID 多行时一次拉取应用到所有行。
     *
     * 入参用 `absPath` 而不是 `pid`：UI 上"重试"按钮属于具体一行；同 PID 多行
     * 时点哪一行的重试都从那行的 PID 拉一次，结果会自动覆盖到所有同 PID 行
     * （它们共享同一份 tags）。
     */
    async retry(absPath: string) {
      const idx = this._pathIndex.get(absPath);
      if (idx === undefined) return;
      const row = this.rows[idx];
      if (!row) return;
      const pid = row.pid;
      const pidIndices = this._pidIndex.get(pid) ?? [idx];

      // 立刻把同 PID 所有行打回 pending（视觉反馈）
      for (const i of pidIndices) {
        const r = this.rows[i];
        if (!r) continue;
        this.rows[i] = { ...r, status: "pending", error: null };
      }
      this._recomputeCounts();

      try {
        const res = await fetchPixivTagSingle(pid);
        for (const i of pidIndices) {
          const cur = this.rows[i];
          if (!cur) continue;
          this.rows[i] = {
            ...cur,
            tags: res.tags,
            translations: res.translations ?? {},
            status: "ok",
            error: null
          };
        }
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        for (const i of pidIndices) {
          const cur = this.rows[i];
          if (!cur) continue;
          this.rows[i] = {
            ...cur,
            tags: [],
            translations: {},
            status: "error",
            error: msg
          };
        }
        throw e;
      } finally {
        this._recomputeCounts();
      }
    },

    /**
     * 一键重试所有失败行。按 PID 去重后串行重试，避免一次性发起几百条 HTTP
     * 把 Pixiv 限流策略打穿（每个 fetchPixivTagSingle 内部已经走共享的
     * next-slot 限速队列，串行 await 让用户看进度更稳）。
     *
     * 中途任何一条失败不会中断整轮，最后一并抛出"X 个 PID 重试失败"汇总。
     */
    async retryFailed() {
      // 收集需要重试的 PID（按 PID 去重；同 PID 多行只重试一次）
      const failedPids = new Set<string>();
      for (const r of this.rows) {
        if (r.status === "error") failedPids.add(r.pid);
      }
      if (failedPids.size === 0) return { tried: 0, failed: 0 };

      let failed = 0;
      let tried = 0;
      for (const pid of failedPids) {
        const indices = this._pidIndex.get(pid);
        if (!indices || indices.length === 0) continue;
        const firstRow = this.rows[indices[0]];
        if (!firstRow) continue;
        tried += 1;
        try {
          await this.retry(firstRow.absPath);
        } catch {
          failed += 1;
        }
      }
      return { tried, failed };
    },

    /**
     * 把行的图片移动到 `<outputDir>/<tag>/`，把 row.absPath 替换为新路径，
     * 并记下 `movedTag` 给 UI 显示"当前位置"。
     *
     * 行**保留**，方便用户继续往别的 tag 文件夹移动同一张图。
     *
     * 入参用 `absPath`（行的稳定主键）—— PID 不再是单行标识，同 PID 多张图
     * 各自有不同的 absPath。这是这次重构修复的核心：以前用 PID 找 idx，
     * 永远只移动第一张，其他点击没反应；现在按 absPath 找具体那一行。
     *
     * 实现细节：移动后用整个对象替换 `this.rows[idx]`——in-place 改字段在
     * Pinia + Vue 3 reactive proxy 嵌套较深时偶发不触发响应，换引用最稳。
     * 然后同步更新 `_pathIndex`：旧 absPath 删掉、新 absPath 指向同一 idx。
     */
    async moveByTag(absPath: string, tag: string) {
      if (!this.outputDir) {
        throw new Error("请先选择输出目录");
      }
      const idx = this._pathIndex.get(absPath);
      if (idx === undefined) {
        throw new Error("行不存在");
      }
      const row = this.rows[idx];
      if (!row) throw new Error("行不存在");
      const newPath = await moveImageByTag(row.absPath, this.outputDir, tag);

      this.rows[idx] = {
        ...row,
        absPath: newPath,
        movedTag: tag
      };
      // path 索引重写：旧 absPath 失效，新 absPath 指向同一行
      this._pathIndex.delete(absPath);
      this._pathIndex.set(newPath, idx);
    },

    /**
     * 单行设置 / 切换译名覆盖。
     *
     * - `global`：跟随全局开关
     * - `translated`：强制译名（缺译名仍回落原 tag）
     * - `original`：强制原 tag
     */
    setRowTranslationOverride(absPath: string, override: PixivTranslationOverride) {
      const idx = this._pathIndex.get(absPath);
      if (idx === undefined) return;
      const row = this.rows[idx];
      if (!row) return;
      if (row.useTranslationOverride === override) return;
      this.rows[idx] = { ...row, useTranslationOverride: override };
    }
  }
});

function createLocalTaskId(prefix: string): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return `${prefix}-${crypto.randomUUID()}`;
  }
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
