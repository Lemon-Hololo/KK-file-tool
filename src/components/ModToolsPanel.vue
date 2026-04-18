<script setup lang="ts">
import { useStorage } from "@vueuse/core";

import ModRenamePanel from "./ModRenamePanel.vue";
import ModOrganizePanel from "./ModOrganizePanel.vue";
import ModScanPanel from "./ModScanPanel.vue";

defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const activeSubTab = useStorage<"rename" | "organize" | "scan">("modToolsTab", "rename");
</script>

<template>
  <div class="mod-tools-panel">
    <el-tabs v-model="activeSubTab" class="mod-tabs">
      <el-tab-pane label="Mod 重命名" name="rename" class="mod-tab-pane">
        <ModRenamePanel :paths="paths" :ensure-normalized-paths="ensureNormalizedPaths" />
      </el-tab-pane>
      <el-tab-pane label="文件夹归类" name="organize" class="mod-tab-pane">
        <ModOrganizePanel :paths="paths" :ensure-normalized-paths="ensureNormalizedPaths" />
      </el-tab-pane>
      <el-tab-pane label="Mod 版本限制扫描" name="scan" class="mod-tab-pane">
        <ModScanPanel :paths="paths" :ensure-normalized-paths="ensureNormalizedPaths" />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<style scoped>
.mod-tools-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.mod-tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.mod-tabs :deep(.el-tabs__content) {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.mod-tab-pane {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}
</style>
