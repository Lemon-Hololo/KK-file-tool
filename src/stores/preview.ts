import { defineStore } from "pinia";
import type { PreviewPayload } from "../types/preview";
import { requestPreview } from "../services/preview";

export const usePreviewStore = defineStore("preview", {
  state: () => ({
    loading: false,
    filePath: "",
    data: null as PreviewPayload | null
  }),
  actions: {
    async open(filePath: string) {
      this.filePath = filePath;
      this.loading = true;
      try {
        this.data = await requestPreview(filePath);
      } finally {
        this.loading = false;
      }
    },
    clear() {
      this.filePath = "";
      this.data = null;
    }
  }
});