/** 文件预览的前端状态管理（悬浮面板使用）。 */

import { defineStore } from "pinia";
import type { PreviewPayload } from "../types/preview";
import { requestPreview } from "../services/preview";

type PreviewStoreState = {
  loading: boolean;
  filePath: string;
  data: PreviewPayload | null;
};

let activePreviewPath = "";
let queuedPreviewPath: string | null = null;
let activePreviewRequest: Promise<PreviewPayload | null> | null = null;

async function drainPreviewQueue(store: PreviewStoreState) {
  let lastError: unknown = null;

  while (queuedPreviewPath) {
    const nextPath = queuedPreviewPath;
    queuedPreviewPath = null;
    activePreviewPath = nextPath;
    store.filePath = nextPath;
    store.loading = true;

    try {
      const data = await requestPreview(nextPath);
      lastError = null;

      // 已经排了新的目标时，旧结果直接丢弃，避免 hover 快速切换时反复回写。
      if (!queuedPreviewPath) {
        store.data = data;
      }
    } catch (error) {
      lastError = error;
      if (!queuedPreviewPath) {
        store.data = null;
      }
    }
  }

  activePreviewPath = "";
  activePreviewRequest = null;
  store.loading = false;

  if (lastError && !store.data) {
    throw lastError;
  }

  return store.data;
}

export const usePreviewStore = defineStore("preview", {
  state: () => ({
    loading: false,
    filePath: "",
    data: null as PreviewPayload | null
  }),
  actions: {
    /**
     * 打开指定文件预览。
     *
     * 同一路径重复悬停直接复用当前结果；不同路径改成串行请求，任意时刻只保留
     * 一个有效预览请求，快速扫过多行时只会继续请求最后一个目标。
     */
    async open(filePath: string) {
      const nextPath = filePath.trim();
      if (!nextPath) {
        this.clear();
        return null;
      }

      if (nextPath === this.filePath && (this.loading || this.data)) {
        return activePreviewRequest ?? this.data;
      }

      if (nextPath === activePreviewPath || nextPath === queuedPreviewPath) {
        return activePreviewRequest ?? this.data;
      }

      if (nextPath !== this.filePath) {
        this.data = null;
      }

      this.filePath = nextPath;
      this.loading = true;
      queuedPreviewPath = nextPath;

      if (!activePreviewRequest) {
        activePreviewRequest = drainPreviewQueue(this);
      }

      return activePreviewRequest;
    },

    /** 清空当前预览展示。 */
    clear() {
      this.filePath = "";
      this.data = null;
      this.loading = false;
      queuedPreviewPath = null;
    }
  }
});
