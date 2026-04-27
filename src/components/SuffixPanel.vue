<script setup lang="ts">
/** 后缀批量修改面板。薄包装，业务流程全部委托给 `OpsPanel`。 */

import { computed } from "vue";
import { useStorage } from "@vueuse/core";

import { useSuffixStore } from "../stores/suffix";
import { useConfigStore } from "../stores/config";
import type { VirtualColumn } from "../types/virtualTable";
import OpsPanel from "./common/OpsPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const suffixStore = useSuffixStore();
const configStore = useConfigStore();

// useStorage 的第二参只在首次（localStorage 无值）时生效，
// 所以全局默认改了以后"老用户"不会被覆盖，符合预期。
const targetSuffix = useStorage<string>(
  "suffixTargetSuffix",
  configStore.settings.suffixDefaultTarget || "txt"
);

const columns: VirtualColumn[] = [
  { key: "oldPath", label: "修改前", minWidth: 320, ellipsis: true, resizable: true },
  { key: "newPath", label: "修改后", minWidth: 320, ellipsis: true, resizable: true },
  { key: "status", label: "状态", width: 100, resizable: true },
  { key: "message", label: "信息", minWidth: 180, ellipsis: true, resizable: true }
];

const rows = computed<any[]>(
  () => (suffixStore.lastApplyResult?.items || suffixStore.previewList) as any[]
);

async function preview(normalized: string[]) {
  await suffixStore.preview(normalized, targetSuffix.value);
}

async function apply(normalized: string[], selected: string[]) {
  const result = await suffixStore.apply(normalized, targetSuffix.value, null, selected);
  await suffixStore.refreshRecords();
  return result;
}

function checkRollback(itemIds?: number[] | null) {
  const id = suffixStore.lastApplyResult?.recordId;
  if (!id) return Promise.resolve({ missingPaths: [] });
  return suffixStore.checkRollback(id, itemIds);
}

function rollback(itemIds?: number[] | null) {
  const id = suffixStore.lastApplyResult?.recordId!;
  return suffixStore.rollback(id, itemIds, true);
}
</script>

<template>
  <OpsPanel
    :paths="props.paths"
    :ensure-normalized-paths="props.ensureNormalizedPaths"
    :columns="columns"
    :rows="rows"
    :apply-items="suffixStore.lastApplyResult?.items ?? null"
    :last-record-id="suffixStore.lastApplyResult?.recordId ?? null"
    apply-confirm-text="确认执行后缀批量修改？"
    column-config-key="task:suffix"
    :preview="preview"
    :apply="apply"
    :check-rollback="checkRollback"
    :rollback="rollback"
  >
    <template #topForm>
      <label class="inline-field">
        <span class="field-label">目标后缀</span>
        <el-input v-model="targetSuffix" placeholder="如 txt 或 .txt" size="default" class="suffix-input" />
      </label>
    </template>
  </OpsPanel>
</template>

<style scoped>
.inline-field {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.field-label {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  font-weight: 500;
}
.suffix-input {
  min-width: 120px;
  max-width: 240px;
}
</style>
