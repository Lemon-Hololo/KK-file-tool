import { defineStore } from "pinia";
import { onEvent } from "../services/tauri";
import { pauseTask, resumeTask, stopTask } from "../services/task";
import type { TaskLogPayload, TaskProgressPayload, TaskStatus } from "../types/common";
import { DEFAULT_LOG_MAX_LENGTH, LOG_FLUSH_INTERVAL } from "../constants/app";
import { useConfigStore } from "./config";

/**
 * 日志缓冲区（非响应式），高频 IPC 事件先写入此处，
 * 由定时器批量刷入响应式 logs 数组，避免逐条触发 Vue 响应式更新。
 */
let _logBuffer: TaskLogPayload[] = [];
let _flushTimer: ReturnType<typeof setInterval> | null = null;

/**
 * 取当前日志上限。优先读配置中心的 `logMaxLength`；用户尚未改过（或 config
 * store 还没初始化）时退到编译期默认。上限硬压在 50 万条防止失控。
 */
function currentLogCap(): number {
  try {
    const configStore = useConfigStore();
    const v = configStore.settings.logMaxLength;
    if (typeof v === "number" && v > 0) return Math.min(v, 500_000);
  } catch {
    /* pinia 未就绪时静默退到默认值 */
  }
  return DEFAULT_LOG_MAX_LENGTH;
}

export const useRuntimeStore = defineStore("runtime", {
  state: () => ({
    taskId: "",
    status: "Idle" as TaskStatus,
    logs: [] as TaskLogPayload[],
    progress: {
      taskId: "",
      stage: "",
      processed: 0,
      total: 0,
      percent: 0
    } as TaskProgressPayload,
    _inited: false
  }),

  actions: {
    /** 追加一条本地日志，并立即刷入，避免同步命令长时间无反馈。 */
    appendLocalLog(taskId: string, level: TaskLogPayload["level"], message: string, filePath?: string) {
      if (this.taskId !== taskId) return;
      _logBuffer.push({ taskId, level, message, filePath });
      this._flushLogs();
    },

    /** 将缓冲区日志批量刷入响应式数组，超限时裁剪旧数据 */
    _flushLogs() {
      if (_logBuffer.length === 0) return;
      const batch = _logBuffer;
      _logBuffer = [];

      // 一次性拼接 + 裁剪，仅触发一次响应式更新
      const cap = currentLogCap();
      const merged = this.logs.concat(batch);
      if (merged.length > cap) {
        // 保留最新的 cap 条
        this.logs = merged.slice(merged.length - cap);
      } else {
        this.logs = merged;
      }
    },

    _startFlushTimer() {
      if (_flushTimer) return;
      _flushTimer = setInterval(() => this._flushLogs(), LOG_FLUSH_INTERVAL);
    },

    _stopFlushTimer() {
      if (_flushTimer) {
        clearInterval(_flushTimer);
        _flushTimer = null;
      }
      // 停止前把剩余缓冲刷入
      this._flushLogs();
    },

    setRunningTask(taskId: string) {
      this.taskId = taskId;
      this.status = "Running";
      this.logs = [];
      _logBuffer = [];
      this.progress = { taskId: "", stage: "", processed: 0, total: 0, percent: 0 };
      this._startFlushTimer();
    },

    finishLocalTask(taskId: string, status: TaskStatus = "Completed") {
      if (this.taskId !== taskId) return;
      this.status = status;
      this._stopFlushTimer();
    },

    failLocalTask(taskId: string, message?: string) {
      if (this.taskId !== taskId) return;
      if (message) {
        this.appendLocalLog(taskId, "ERROR", message);
      }
      this.status = "Failed";
      this._stopFlushTimer();
    },

    async initEvents() {
      if (this._inited) return;
      this._inited = true;

      await onEvent<TaskLogPayload>("task_log", (payload) => {
        if (this.taskId && payload.taskId !== this.taskId) return;
        // 写入非响应式缓冲区，不触发 Vue 更新
        _logBuffer.push(payload);
      });

      await onEvent<TaskProgressPayload>("task_progress", (payload) => {
        if (this.taskId && payload.taskId !== this.taskId) return;
        this.progress = payload;
      });

      await onEvent<{ taskId: string; status: TaskStatus }>("task_state_changed", (payload) => {
        if (this.taskId && payload.taskId !== this.taskId) return;
        this.status = payload.status;
        // 任务结束时停止定时器并刷入剩余日志
        if (payload.status !== "Running" && payload.status !== "Paused") {
          this._stopFlushTimer();
        }
      });

      await onEvent<{ taskId: string; message: string }>("task_failed", (payload) => {
        if (this.taskId && payload.taskId !== this.taskId) return;
        this.status = "Failed";
        this._stopFlushTimer();
      });
    },

    pause() {
      return this.taskId ? pauseTask(this.taskId) : Promise.resolve();
    },

    resume() {
      return this.taskId ? resumeTask(this.taskId) : Promise.resolve();
    },

    stop() {
      return this.taskId ? stopTask(this.taskId) : Promise.resolve();
    },

    clearLogs() {
      this.logs = [];
      _logBuffer = [];
    }
  }
});
