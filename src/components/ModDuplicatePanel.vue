<script setup lang="ts">
/**
 * 重复 MOD 检查面板。结构与 [`ModVersionPanel`](./ModVersionPanel.vue) 高度
 * 雷同，共用实现见 [`ModGroupPanel`](./ModGroupPanel.vue)。
 */
import ModGroupPanel from "./ModGroupPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();
</script>

<template>
  <ModGroupPanel kind="duplicate" v-bind="props" />
</template>
