<script setup lang="ts">
/**
 * 通用"可撤回操作"面板。
 *
 * 把"预览 → 选择 → 应用 → 撤回（全部 / 选中）"的交互流固定下来；
 * 具体业务（后缀修改 / Mod 重命名 / Mod 归类）通过 props 传入回调和表头。
 * 面板内部不持有业务状态，全部通过回调与父组件/store 同步。
 *
 * # 布局
 * 用自有 `Panel` 作为壳（替代 el-card），body 默认 padded，从上到下：
 * - 顶部控件区（el-form-item + 按钮组）——`flex-shrink: 0`
 * - 可选 info 提示（自己写的简洁样式，不再依赖 el-alert）——`flex-shrink: 0`
 * - `VirtualTable`（auto-height 模式）——`flex: 1; min-height: 0`
 *
 * 这里不再用 `el-card` 的原因：`el-card__body` 的默认 padding 与我们想要的
 * 12px flex gap 不一致，用 `:deep()` 覆写又容易随 EP 升级失效。自有 Panel 把
 * 布局契约定死，调用方无需再担心"卡片 body 能不能撑开"这种事。
 */

import { computed, ref, useSlots } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";

import {
  DEFAULT_EXTREME_ROW_THRESHOLD,
  EXTREME_OVERSCAN,
  NORMAL_OVERSCAN
} from "../../constants/task";
import { useConfigStore } from "../../stores/config";
import type { VirtualColumn } from "../../types/virtualTable";
import Panel from "./Panel.vue";
import VirtualTable from "./VirtualTable.vue";

interface RowBase {
  oldPath: string;
  [k: string]: unknown;
}

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
  columns: VirtualColumn[];
  rowKey?: string;
  preview: (normalizedPaths: string[]) => Promise<void>;
  apply: (
    normalizedPaths: string[],
    selectedOldPaths: string[]
  ) => Promise<{ success: number; failed: number }>;
  checkRollback: (itemIds?: number[] | null) => Promise<{ missingPaths: string[] }>;
  rollback: (
    itemIds?: number[] | null
  ) => Promise<{ success: number; failed: number; skippedMissing: number }>;
  rows: RowBase[];
  applyItems: Array<{ oldPath: string; itemId: number }> | null | undefined;
  lastRecordId: string | null | undefined;
  applyConfirmText?: string;
  applyButtonText?: string;
  previewToastBuilder?: (count: number) => string;
  applySelectionFilter?: (row: RowBase) => boolean;
  infoTip?: string;
  /** 列配置持久化 key，跨组件透传 */
  columnConfigKey?: string;
  /** 面板运行中时统一禁用动作按钮。 */
  busy?: boolean;
}>();

const configStore = useConfigStore();
const slots = useSlots();

const selectedOldPaths = ref<string[]>([]);

const isExtreme = computed(() => {
  const th = configStore.settings.extremeRowThreshold || DEFAULT_EXTREME_ROW_THRESHOLD;
  return props.rows.length > th;
});

const tableSlotNames = computed(() => Object.keys(slots).filter((name) => name !== "topForm"));

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
  <Panel class="ops-panel" :padded="true">
    <div class="ops-toolbar">
      <div class="ops-inputs">
        <slot name="topForm" />
      </div>
      <div class="ops-actions">
        <el-button :disabled="busy" @click="handlePreview">预览</el-button>
        <el-button type="primary" :disabled="busy" @click="handleApply">
          {{ applyButtonText ?? "确认修改" }}
        </el-button>
        <el-button type="warning" plain :disabled="busy" @click="handleRollbackLast">撤回本次</el-button>
        <el-button plain :disabled="busy" @click="handleRollbackSelected">撤回选中</el-button>
      </div>
    </div>

    <div class="ops-tip">
      <span class="dot" />
      <span>{{
        infoTip ?? `当前勾选：${selectedOldPaths.length}（未勾选时默认处理全部有效预览项）`
      }}</span>
    </div>

    <div v-if="isExtreme" class="ops-warn">
      <span class="dot dot-warn" />
      <span>数据量较大，已启用极限性能模式</span>
    </div>

    <VirtualTable
      class="ops-table"
      :rows="rows"
      :columns="columns"
      :item-height="36"
      :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
      :row-key="rowKey ?? 'oldPath'"
      :column-config-key="columnConfigKey"
      fit-width
      selectable
      @selectionChange="onSelectionChange"
    >
      <template v-for="name in tableSlotNames" :key="name" #[name]="scope">
        <slot :name="name" v-bind="scope" />
      </template>
    </VirtualTable>
  </Panel>
</template>

<style scoped>
.ops-panel {
  height: 100%;
  min-height: 0;
}

.ops-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: var(--ff-space-2) var(--ff-space-3);
  align-items: center;
  flex-shrink: 0;
}
.ops-inputs {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2) var(--ff-space-3);
  min-width: 0;
}
.ops-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2);
  margin-left: auto;
  flex-shrink: 0;
}

.ops-tip,
.ops-warn {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--ff-font-sm);
  padding: 6px 10px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-accent-soft);
  color: var(--ff-text-secondary);
  flex-shrink: 0;
}
.ops-warn {
  background: var(--ff-warning-soft);
  color: var(--ff-warning);
}
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ff-accent);
  flex-shrink: 0;
}
.dot-warn {
  background: var(--ff-warning);
}

.ops-table {
  flex: 1;
  min-height: 0;
}

/* 让 el-form-item 在 toolbar 里跟我们的 gap 对齐，不再用 EP 自己的 margin */
.ops-toolbar :deep(.el-form-item) {
  margin: 0;
}
</style>
