<script setup lang="ts">
/**
 * 空文件夹清理面板。薄包装，业务流程全部委托给 `OpsPanel`。
 *
 * 删除时写入可撤回记录；撤回语义是重新创建这些空目录。
 */

import { computed } from "vue";
import { useStorage } from "@vueuse/core";

import { useEmptyDirsStore } from "../stores/emptyDirs";
import type { EmptyDirApplyItem, EmptyDirPreviewItem } from "../types/emptyDirs";
import type { VirtualColumn } from "../types/virtualTable";
import OpsPanel from "./common/OpsPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useEmptyDirsStore();
const includeRoots = useStorage<boolean>("emptyDirsIncludeRoots", false);

type EmptyDirRow = (EmptyDirPreviewItem | EmptyDirApplyItem) & Record<string, unknown>;

const columns: VirtualColumn[] = [
  { key: "oldPath", label: "空文件夹", minWidth: 420, ellipsis: true, resizable: true },
  {
    key: "depth",
    label: "层级",
    width: 90,
    resizable: true,
    formatter: (_row, value) => (value == null ? "" : String(value))
  },
  {
    key: "status",
    label: "状态",
    width: 100,
    resizable: true,
    formatter: (_row, value) => {
      if (value === "success") return "已删除";
      if (value === "failed") return "失败";
      return value ? String(value) : "待删除";
    }
  },
  { key: "message", label: "信息", minWidth: 220, ellipsis: true, resizable: true }
];

const rows = computed<EmptyDirRow[]>(
  () => (store.lastApplyResult?.items || store.previewList) as EmptyDirRow[]
);

async function preview(normalized: string[]) {
  await store.preview(normalized, includeRoots.value);
}

async function apply(normalized: string[], selected: string[]) {
  const result = await store.apply(normalized, includeRoots.value, null, selected);
  await store.refreshRecords();
  return result;
}

function checkRollback(itemIds?: number[] | null) {
  const id = store.lastApplyResult?.recordId;
  if (!id) return Promise.resolve({ missingPaths: [] });
  return store.checkRollback(id, itemIds);
}

function rollback(itemIds?: number[] | null) {
  const id = store.lastApplyResult?.recordId!;
  return store.rollback(id, itemIds, true);
}
</script>

<template>
  <OpsPanel
    :paths="props.paths"
    :ensure-normalized-paths="props.ensureNormalizedPaths"
    :columns="columns"
    :rows="rows"
    :apply-items="store.lastApplyResult?.items ?? null"
    :last-record-id="store.lastApplyResult?.recordId ?? null"
    apply-button-text="确认删除"
    apply-confirm-text="确认删除这些空文件夹？删除后可通过记录重新创建空目录。"
    column-config-key="task:empty-dirs"
    info-tip="默认不删除任务输入根目录；未勾选时默认删除全部候选项。勾选父目录会连同它下面的空子目录一起处理。"
    :preview-toast-builder="(count) => `预览完成，共 ${count} 个空文件夹`"
    :preview="preview"
    :apply="apply"
    :check-rollback="checkRollback"
    :rollback="rollback"
  >
    <template #topForm>
      <el-checkbox v-model="includeRoots">包含任务根目录</el-checkbox>
    </template>
  </OpsPanel>
</template>
