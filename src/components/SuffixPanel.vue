<script setup lang="ts">
/** 后缀批量修改面板。薄包装，业务流程全部委托给 `OpsPanel`。 */

import { computed } from "vue";
import { useStorage } from "@vueuse/core";

import { useSuffixStore } from "../stores/suffix";
import type { VirtualColumn } from "../types/virtualTable";
import OpsPanel from "./common/OpsPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const suffixStore = useSuffixStore();

const targetSuffix = useStorage<string>("suffixTargetSuffix", "txt");

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
    :preview="preview"
    :apply="apply"
    :check-rollback="checkRollback"
    :rollback="rollback"
  >
    <template #topForm>
      <el-form-item label="目标后缀">
        <el-input v-model="targetSuffix" placeholder="如 txt 或 .txt" />
      </el-form-item>
    </template>
  </OpsPanel>
</template>
