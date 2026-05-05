<script setup lang="ts">
/**
 * 不同版本 MOD 检查面板。结构与 [`ModDuplicatePanel`](./ModDuplicatePanel.vue) 共用，
 * 实现见 [`ModGroupPanel`](./ModGroupPanel.vue)。
 */
import ModGroupPanel from "./ModGroupPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();
</script>

<template>
  <ModGroupPanel kind="version" v-bind="props" />
</template>
