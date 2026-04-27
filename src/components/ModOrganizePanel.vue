<script setup lang="ts">
/** Mod 归类面板。薄包装，业务流程全部委托给 `OpsPanel`。 */

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
  { key: "oldPath", label: "源文件", minWidth: 320, ellipsis: true, resizable: true },
  { key: "folderName", label: "目标子目录", width: 180, ellipsis: true, resizable: true },
  { key: "newPath", label: "目标路径", minWidth: 320, ellipsis: true, resizable: true },
  { key: "status", label: "状态", width: 90, resizable: true },
  { key: "message", label: "信息", minWidth: 180, ellipsis: true, resizable: true },
  { key: "__actions", label: "目录", width: 92, slotName: "actions" }
];

const rows = computed<any[]>(
  () => (store.organizeApplyResult?.items || store.organizePreview) as any[]
);

async function preview(normalized: string[]) {
  await store.previewOrganize(normalized);
}

async function apply(normalized: string[], selected: string[]) {
  const result = await store.applyOrganize(normalized, null, selected);
  await store.refreshRecords();
  return result;
}

function checkRollback(itemIds?: number[] | null) {
  const id = store.organizeApplyResult?.recordId;
  if (!id) return Promise.resolve({ missingPaths: [] });
  return store.checkRollback(id, itemIds);
}

function rollback(itemIds?: number[] | null) {
  const id = store.organizeApplyResult?.recordId!;
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
    :apply-items="store.organizeApplyResult?.items ?? null"
    :last-record-id="store.organizeApplyResult?.recordId ?? null"
    apply-button-text="确认归类"
    apply-confirm-text="确认执行文件夹归类？"
    column-config-key="task:mod-organize"
    :busy="store.busy.organize"
    info-tip="对任务输入的每个文件夹按首个 [...] 括号建子目录归类（非递归）。未勾选时默认处理全部预览项。"
    :preview="preview"
    :apply="apply"
    :check-rollback="checkRollback"
    :rollback="rollback"
  >
    <template #actions="{ row }">
      <el-button size="small" text :disabled="store.busy.organize" @click.stop="openFolder(row.oldPath)">
        打开
      </el-button>
    </template>
  </OpsPanel>
</template>
