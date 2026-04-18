<script setup lang="ts">
/**
 * 通用"可撤回操作"面板。
 *
 * 把"预览 → 选择 → 应用 → 撤回（全部 / 选中）"的交互流固定下来；
 * 具体业务（后缀修改 / Mod 重命名 / Mod 归类）通过 props 传入回调和表头。
 * 面板内部不持有业务状态，全部通过回调与父组件/store 同步。
 */

import { computed, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useElementSize } from "@vueuse/core";

import {
  EXTREME_OVERSCAN,
  EXTREME_SUFFIX_ROW_THRESHOLD,
  NORMAL_OVERSCAN
} from "../../constants/task";
import type { VirtualColumn } from "../../types/virtualTable";
import VirtualTable from "./VirtualTable.vue";

interface RowBase {
  oldPath: string;
  [k: string]: unknown;
}

const props = defineProps<{
  /** 当前任务输入路径（仅透传给回调）。 */
  paths: string[];
  /** 调用后端 `normalize_input_paths` 并 persist 结果；`null` 表示无可用路径。 */
  ensureNormalizedPaths: () => Promise<string[] | null>;
  /** 表格列定义。 */
  columns: VirtualColumn[];
  /** 行唯一键，默认 `oldPath`。 */
  rowKey?: string;
  /** 预览回调：负责把结果写入 store。 */
  preview: (normalizedPaths: string[]) => Promise<void>;
  /** 应用回调：`selectedOldPaths` 为空时视为"处理全部有效项"。 */
  apply: (
    normalizedPaths: string[],
    selectedOldPaths: string[]
  ) => Promise<{ success: number; failed: number }>;
  /** 撤回前检查缺失文件（支持 `itemIds` 过滤）。 */
  checkRollback: (itemIds?: number[] | null) => Promise<{ missingPaths: string[] }>;
  /** 执行撤回。 */
  rollback: (
    itemIds?: number[] | null
  ) => Promise<{ success: number; failed: number; skippedMissing: number }>;
  /** 当前用于表格渲染的行（预览或应用结果，由父组件二选一）。 */
  rows: RowBase[];
  /** 应用结果 items，用于"撤回选中"把 `oldPath` 映射回 `itemId`。 */
  applyItems: Array<{ oldPath: string; itemId: number }> | null | undefined;
  /** 最后一次应用得到的 `recordId`；无则禁用撤回按钮。 */
  lastRecordId: string | null | undefined;
  /** 应用前的确认文案。 */
  applyConfirmText?: string;
  /** 应用按钮文案。 */
  applyButtonText?: string;
  /** 预览后的 toast 构造函数（默认展示条目数）。 */
  previewToastBuilder?: (count: number) => string;
  /** 额外的"选中筛选"：rename 面板用此过滤掉 `warn` 非空项。 */
  applySelectionFilter?: (row: RowBase) => boolean;
  /** 顶部信息提示。 */
  infoTip?: string;
}>();

const panelRef = ref<HTMLElement | null>(null);
const { height: panelHeight } = useElementSize(panelRef);

const selectedOldPaths = ref<string[]>([]);

const tableHeight = computed(() => {
  const h = panelHeight.value;
  if (!h) return 420;
  return Math.max(260, h - 180);
});

const isExtreme = computed(() => props.rows.length > EXTREME_SUFFIX_ROW_THRESHOLD);

function onSelectionChange(rows: RowBase[]) {
  selectedOldPaths.value = rows.map((x) => x.oldPath).filter(Boolean);
}

async function handlePreview() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  await props.preview(normalized);
  selectedOldPaths.value = [];
  const toast = props.previewToastBuilder
    ? props.previewToastBuilder(props.rows.length)
    : `预览完成，共 ${props.rows.length} 条`;
  ElMessage.success(toast);
}

async function handleApply() {
  if (!props.rows.length) {
    ElMessage.warning("请先执行预览");
    return;
  }
  await ElMessageBox.confirm(props.applyConfirmText ?? "确认执行此批处理？", "确认", {
    type: "warning"
  });
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;

  let selected: string[];
  if (selectedOldPaths.value.length) {
    selected = selectedOldPaths.value;
  } else {
    const filter = props.applySelectionFilter ?? (() => true);
    selected = props.rows.filter(filter).map((x) => x.oldPath);
  }

  const result = await props.apply(normalized, selected);
  ElMessage.success(`完成：成功 ${result.success}，失败 ${result.failed}`);
  selectedOldPaths.value = [];
}

async function confirmMissing(count: number, prefix = "") {
  if (!count) return;
  await ElMessageBox.confirm(
    `${prefix}有 ${count} 个文件不存在，仅撤回存在文件，继续？`,
    "缺失提示",
    { type: "warning" }
  );
}

async function handleRollbackLast() {
  if (!props.lastRecordId) {
    ElMessage.warning("没有可撤回记录");
    return;
  }
  const check = await props.checkRollback(null);
  await confirmMissing(check.missingPaths.length);
  const resp = await props.rollback(null);
  ElMessage.success(
    `撤回完成：成功 ${resp.success}，失败 ${resp.failed}，跳过缺失 ${resp.skippedMissing}`
  );
}

async function handleRollbackSelected() {
  if (!props.lastRecordId) {
    ElMessage.warning("没有可撤回记录");
    return;
  }
  const map = new Map((props.applyItems ?? []).map((x) => [x.oldPath, x.itemId]));
  const itemIds = selectedOldPaths.value
    .map((p) => map.get(p))
    .filter((x): x is number => typeof x === "number");

  if (!itemIds.length) {
    ElMessage.warning("请先勾选要撤回的记录项");
    return;
  }
  const check = await props.checkRollback(itemIds);
  await confirmMissing(check.missingPaths.length, "选中项中");
  const resp = await props.rollback(itemIds);
  ElMessage.success(
    `部分撤回完成：成功 ${resp.success}，失败 ${resp.failed}，跳过缺失 ${resp.skippedMissing}`
  );
}
</script>

<template>
  <div ref="panelRef" class="ops-panel">
    <el-card shadow="hover" class="main-card">
      <el-form inline class="top-form">
        <slot name="topForm" />
        <el-form-item>
          <el-space wrap>
            <el-button @click="handlePreview">预览</el-button>
            <el-button type="primary" @click="handleApply">
              {{ applyButtonText ?? "确认修改" }}
            </el-button>
            <el-button type="warning" @click="handleRollbackLast">撤回本次</el-button>
            <el-button @click="handleRollbackSelected">撤回选中</el-button>
          </el-space>
        </el-form-item>
      </el-form>

      <el-alert
        type="info"
        :closable="false"
        style="margin-bottom:8px"
        :title="infoTip ?? `当前勾选：${selectedOldPaths.length}（未勾选时默认处理全部有效预览项）`"
      />

      <el-alert
        v-if="isExtreme"
        type="warning"
        :closable="false"
        style="margin-bottom:8px"
        title="数据量较大，已启用极限性能模式"
      />

      <VirtualTable
        :rows="rows"
        :columns="columns"
        :height="tableHeight"
        :item-height="36"
        :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
        :row-key="rowKey ?? 'oldPath'"
        selectable
        @selectionChange="onSelectionChange"
      />
    </el-card>
  </div>
</template>

<style scoped>
.ops-panel {
  height: 100%;
  min-height: 0;
}
.main-card {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
</style>
