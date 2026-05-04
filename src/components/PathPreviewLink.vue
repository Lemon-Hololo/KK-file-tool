<script setup lang="ts">
/**
 * 路径单元格链接：可选 hover 预览，点击在资源管理器中定位。
 *
 * 多个表格都会渲染"路径文本 + PreviewPanel + revealInExplorer"这组交互；
 * 统一在这里避免每个面板重复写按钮样式、长路径去前缀和 IPC 调用。
 */
import { computed } from "vue";

import { revealInExplorer } from "../services/task";
import { stripWindowsExtendedPrefix } from "../utils/path";
import PreviewPanel from "./PreviewPanel.vue";

const props = withDefaults(
  defineProps<{
    path: string;
    label?: string;
    disabled?: boolean;
    preview?: boolean;
  }>(),
  {
    label: undefined,
    disabled: false,
    preview: true
  }
);

const displayPath = computed(() => stripWindowsExtendedPrefix(props.path ?? ""));
const displayLabel = computed(() => props.label ?? displayPath.value);

async function openPath() {
  if (props.disabled || !props.path) return;
  await revealInExplorer(props.path);
}
</script>

<template>
  <PreviewPanel v-if="preview" :path="path">
    <button
      type="button"
      class="path-link"
      :disabled="disabled"
      :title="displayPath"
      @click.stop="openPath"
    >
      {{ displayLabel }}
    </button>
  </PreviewPanel>
  <button
    v-else
    type="button"
    class="path-link"
    :disabled="disabled"
    :title="displayPath"
    @click.stop="openPath"
  >
    {{ displayLabel }}
  </button>
</template>

<style scoped>
.path-link {
  display: block;
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  color: var(--ff-accent);
  cursor: pointer;
  font: inherit;
}

.path-link:hover {
  text-decoration: underline;
}

.path-link:disabled {
  color: var(--ff-text-muted);
  cursor: not-allowed;
  text-decoration: none;
}
</style>
