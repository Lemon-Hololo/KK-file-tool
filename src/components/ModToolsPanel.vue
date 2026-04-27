<script setup lang="ts">
/**
 * Mod 工具聚合面板：五个子 Tab 切换 —— 重命名 / 归类 / 重复检查 /
 * 不同版本 / 版本限制扫描。
 *
 * 不再渲染自己的 TabBar，由父组件 TaskPage 直接在主 TabBar 中显示子 tab，
 * 本组件只负责渲染三个子面板的容器。
 */
import ModRenamePanel from "./ModRenamePanel.vue";
import ModOrganizePanel from "./ModOrganizePanel.vue";
import ModDuplicatePanel from "./ModDuplicatePanel.vue";
import ModVersionPanel from "./ModVersionPanel.vue";
import ModScanPanel from "./ModScanPanel.vue";

defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
  activeSubTab: "rename" | "organize" | "duplicates" | "versions" | "scan";
}>();
</script>

<template>
  <div class="mod-tools-panel">
    <div class="sub-panel-host">
      <ModRenamePanel
        v-show="activeSubTab === 'rename'"
        :paths="paths"
        :ensure-normalized-paths="ensureNormalizedPaths"
      />
      <ModOrganizePanel
        v-show="activeSubTab === 'organize'"
        :paths="paths"
        :ensure-normalized-paths="ensureNormalizedPaths"
      />
      <ModDuplicatePanel
        v-show="activeSubTab === 'duplicates'"
        :paths="paths"
        :ensure-normalized-paths="ensureNormalizedPaths"
      />
      <ModVersionPanel
        v-show="activeSubTab === 'versions'"
        :paths="paths"
        :ensure-normalized-paths="ensureNormalizedPaths"
      />
      <ModScanPanel
        v-show="activeSubTab === 'scan'"
        :paths="paths"
        :ensure-normalized-paths="ensureNormalizedPaths"
      />
    </div>
  </div>
</template>

<style scoped>
.mod-tools-panel {
  height: 100%;
  min-height: 0;
}

/* sub-panel-host 是 relative 容器，内部每个子面板都 position: absolute
   铺满它。这样 tab 切换时布局不参与 reflow，避免 VirtualTable 被反复
   "看到宽高变化" 而触发 ResizeObserver 闪烁。 */
.sub-panel-host {
  height: 100%;
  min-height: 0;
  position: relative;
}
.sub-panel-host > * {
  position: absolute;
  inset: 0;
}
</style>
