import { defineStore } from "pinia";
import type { AppSettings, DbPathInfo } from "../types/settings";
import { getSettings, saveSettings, setThemeMode, getDbInfo, setCustomDbPath, deleteDatabase, getCpuCount } from "../services/settings";
import { useTheme } from "../composables/useTheme";

export const useConfigStore = defineStore("config", {
  state: () => ({
    settings: {
      keepPolicy: "newest",
      moveTargetPath: "",
      saveRecordEnabled: true,
      useLastRecordEnabled: false,
      includeCurrentFolderDuplicates: true,
      themeMode: "system",
      threadCount: 0,
    } as AppSettings,

    dbPathInfo: null as DbPathInfo | null,
    cpuCount: 0,
  }),

  actions: {
    async loadSettings() {
      const { setThemeMode, initTheme } = useTheme();
      this.settings = await getSettings();

      // 同步主题到 VueUse 主题系统
      setThemeMode(this.settings.themeMode);
      initTheme();
    },

    async saveSettings() {
      await saveSettings(this.settings);

      const { setThemeMode } = useTheme();
      setThemeMode(this.settings.themeMode);
    },

    async changeTheme(mode: AppSettings["themeMode"]) {
      const { setThemeMode } = useTheme();
      this.settings.themeMode = mode;

      // 1) 本地立即生效
      setThemeMode(mode);

      // 2) 后端持久化
      await setThemeModeRemote(mode);
    },

    async loadDbInfo() {
      this.dbPathInfo = await getDbInfo();
    },

    async setCustomDbPath(path: string) {
      await setCustomDbPath(path);
      await this.loadDbInfo();
    },

    async deleteDb() {
      await deleteDatabase();
      await this.loadSettings();
    },

    async loadCpuCount() {
      this.cpuCount = await getCpuCount();
    },
  },
});

// 避免与 composable 的 setThemeMode 命名冲突
async function setThemeModeRemote(mode: AppSettings["themeMode"]) {
  await setThemeMode(mode);
}
