<script setup lang="ts">
/** Mod 重命名面板。薄包装，业务流程全部委托给 `OpsPanel`。 */

import { computed } from "vue";

import { revealInExplorer } from "../services/task";
import { useModToolsStore } from "../stores/modTools";
import type { VirtualColumn } from "../types/virtualTable";
import OpsPanel from "./common/OpsPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useModToolsStore();

const columns: VirtualColumn[] = [
  { key: "oldPath", label: "原文件", minWidth: 260, ellipsis: true, resizable: true },
  { key: "author", label: "作者", width: 140, ellipsis: true, resizable: true },
  { key: "guid", label: "GUID", minWidth: 160, ellipsis: true, resizable: true },
  { key: "version", label: "版本", width: 110, resizable: true },
  { key: "newPath", label: "新文件", minWidth: 260, ellipsis: true, resizable: true },
  { key: "warn", label: "提示", width: 140, ellipsis: true, resizable: true },
  { key: "status", label: "状态", width: 90, resizable: true },
  { key: "message", label: "信息", minWidth: 140, ellipsis: true, resizable: true },
  { key: "__actions", label: "目录", width: 92, slotName: "actions" }
];

const rows = computed<any[]>(
  () => (store.renameApplyResult?.items || store.renamePreview) as any[]
);

async function preview(normalized: string[]) {
  await store.previewRename(normalized);
}

async function apply(normalized: string[], selected: string[]) {
  const result = await store.applyRename(normalized, null, selected);
  await store.refreshRecords();
  return result;
}

function checkRollback(itemIds?: number[] | null) {
  const id = store.renameApplyResult?.recordId;
  if (!id) return Promise.resolve({ missingPaths: [] });
  return store.checkRollback(id, itemIds);
}

function rollback(itemIds?: number[] | null) {
  const id = store.renameApplyResult?.recordId!;
  return store.rollback(id, itemIds, true);
}

async function openFolder(path: string) {
  await revealInExplorer(path);
}
</script>

<template>
  <OpsPanel
    :paths="props.paths"
    :ensure-normalized-paths="props.ensureNormalizedPaths"
    :columns="columns"
    :rows="rows"
    :apply-items="store.renameApplyResult?.items ?? null"
    :last-record-id="store.renameApplyResult?.recordId ?? null"
    apply-button-text="确认重命名"
    apply-confirm-text="确认执行 Mod 重命名？"
    column-config-key="task:mod-rename"
    :busy="store.busy.rename"
    :apply-selection-filter="(row: any) => !row.warn"
    :preview="preview"
    :apply="apply"
    :check-rollback="checkRollback"
    :rollback="rollback"
  >
    <template #actions="{ row }">
      <el-button size="small" text :disabled="store.busy.rename" @click.stop="openFolder(row.oldPath)">
        打开
      </el-button>
    </template>
  </OpsPanel>
</template>
