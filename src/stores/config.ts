/**
 * 应用设置与数据库路径信息的前端状态管理。
 *
 * 设置加载时同步到 `useTheme` composable，保证主题立即切换；
 * 保存同步写入 SQLite。
 */

import { defineStore } from "pinia";
import type { AppSettings, DbPathInfo } from "../types/settings";
import {
  getSettings,
  saveSettings as saveSettingsRemote,
  getDbInfo,
  setCustomDbPath,
  deleteDatabase,
  getCpuCount
} from "../services/settings";
import { useTheme } from "../composables/useTheme";

let queuedSettingsSnapshot: AppSettings | null = null;
let activeSettingsSave: Promise<void> | null = null;

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
      // 性能
      logMaxLength: 3000,
      ioConcurrencyMultiplier: 2,
      extremeRowThreshold: 20000,
      // 预览
      textPreviewMaxKb: 256,
      zipPreviewMaxEntries: 5000,
      // 工具默认值
      modScanDefaultKeyword: "Koikatsu",
      suffixDefaultTarget: "txt",
      // Mod 工具回滚
      modRollbackEnabled: true,
      modBackupDir: "",
      // Pixiv 标签整理
      pixivTagApiBase: "https://www.pixiv.net/ajax/illust/",
      pixivExcludedTags: [],
      pixivLocalTagTranslations: {},
      pixivCookie: "",
      pixivProxy: "",
      pixivUseTranslation: false,
      pixivRateLimitPerMinute: 60,
      pixivPartialFlushIntervalMs: 0,
    } as AppSettings,

    dbPathInfo: null as DbPathInfo | null,
    cpuCount: 0,
  }),

  actions: {
    /** 从后端加载当前配置，并同步主题到本地。 */
    async loadSettings() {
      const { setThemeMode, initTheme } = useTheme();
      this.settings = await getSettings();

      // 同步主题到 VueUse 主题系统
      setThemeMode(this.settings.themeMode);
      initTheme();
    },

    /** 把当前配置排队写入后端；多次快速修改会自动合并为最终一次。 */
    async saveSettings() {
      queuedSettingsSnapshot = cloneSettings(this.settings);
      await flushQueuedSettings();
    },

    /** 仅本地立即切换主题；持久化交给自动保存。 */
    applyThemeMode(mode: AppSettings["themeMode"]) {
      const { setThemeMode } = useTheme();
      this.settings.themeMode = mode;
      setThemeMode(mode);
    },

    /** 读取当前 / 默认 / 自定义数据库路径。 */
    async loadDbInfo() {
      this.dbPathInfo = await getDbInfo();
    },

    /** 设置自定义数据库路径，并刷新路径信息。 */
    async setCustomDbPath(path: string) {
      await setCustomDbPath(path);
      await this.loadDbInfo();
    },

    /** 删除数据库并重新加载默认配置。 */
    async deleteDb() {
      await deleteDatabase();
      await this.loadSettings();
    },

    /** 读取本机 CPU 核心数。 */
    async loadCpuCount() {
      this.cpuCount = await getCpuCount();
    },
  },
});

function cloneSettings(settings: AppSettings): AppSettings {
  return JSON.parse(JSON.stringify(settings)) as AppSettings;
}

async function flushQueuedSettings() {
  if (activeSettingsSave) {
    return activeSettingsSave;
  }

  activeSettingsSave = (async () => {
    while (queuedSettingsSnapshot) {
      const snapshot = queuedSettingsSnapshot;
      queuedSettingsSnapshot = null;
      await saveSettingsRemote(snapshot);

      const { setThemeMode } = useTheme();
      setThemeMode(snapshot.themeMode);
    }
  })();

  try {
    await activeSettingsSave;
  } finally {
    activeSettingsSave = null;
  }
}
