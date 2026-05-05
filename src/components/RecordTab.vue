<script setup lang="ts">
/**
 * 通用"记录管理"标签页组件。
 *
 * 把"标题 + 搜索框 + 批删按钮 + VirtualTable + 详情抽屉"这一组结构收口。
 * 后缀 / 空文件夹 / Mod 三个 tab 走这条路径，差异通过 props 注入。
 *
 * # 与 OpsPanel 的区别
 * OpsPanel 是"任务面板"——预览 → 应用 → 撤回，操作的是当前一批待执行工作。
 * RecordTab 是"记录管理"——历史记录的列表 / 详情 / 撤回 / 删除，操作的是已落
 * 库的记录。两条路径共用 `OpRecordSummary<Extra>` 类型但 UI 流程不同。
 *
 * # 自定义操作列
 * 默认操作列只有"详情 + 撤回"。父组件可通过 `#rowActions` slot 加额外按钮
 * （Mod 记录的"重命名"等）。
 *
 * # 头部过滤器
 * 父组件可通过 `#headerExtra` slot 在搜索框前插入额外控件（如 Mod 记录的
 * kind 过滤 segmented 控件），并把过滤逻辑通过 `extraFilter` 注入。
 *
 * # 详情抽屉
 * 内置 el-drawer + el-descriptions + 内嵌 VirtualTable 详情。父组件通过
 * `#extraDescription` slot 注入业务专属字段。
 */
import { computed, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";

import Panel from "./common/Panel.vue";
import VirtualTable from "./common/VirtualTable.vue";
import type { VirtualColumn } from "../types/virtualTable";
import type { OpRollbackCheck, OpRollbackResponse } from "../types/opRecord";
import { confirmMissingPaths } from "../composables/useDangerConfirm";
import { formatRollbackToast } from "../utils/format";

interface RecordSummaryLike {
  recordId: string;
  recordName: string;
  rollbackEnabled: boolean;
  [k: string]: unknown;
}

interface RecordDetailLike {
  summary: RecordSummaryLike;
  items: unknown[];
}

interface RecordStoreLike {
  records: RecordSummaryLike[];
  currentDetail: RecordDetailLike | null;
  refreshRecords: (kind?: string | null) => Promise<void>;
  loadDetail: (recordId: string) => Promise<unknown>;
  removeBatch: (recordIds: string[]) => Promise<void>;
  checkRollback: (recordId: string, itemIds?: number[] | null) => Promise<OpRollbackCheck>;
  rollback: (
    recordId: string,
    itemIds?: number[] | null,
    forceIgnoreMissing?: boolean
  ) => Promise<OpRollbackResponse>;
}

const props = defineProps<{
  /** 业务 store；至少实现 RecordStoreLike 接口。 */
  store: RecordStoreLike;
  /** 列表表头（不含操作列）；操作列由本组件追加。 */
  listColumns: VirtualColumn[];
  /** 详情抽屉里 VirtualTable 的列表头。 */
  detailColumns: VirtualColumn[];
  /** Panel 标题。 */
  panelTitle: string;
  /** 详情抽屉标题。 */
  detailTitle: string;
  /** 搜索框占位符。 */
  searchPlaceholder: string;
  /** 列表列自定义持久化 key。 */
  listColumnConfigKey: string;
  /** 详情列自定义持久化 key。 */
  detailColumnConfigKey: string;
  /** 计算"可被搜索的字符串"的回调；默认拼 recordName + recordId。 */
  searchableFields?: (record: RecordSummaryLike) => string;
  /** 过滤器（如 Mod 记录的 kind 过滤）。返回 true 表示保留。 */
  extraFilter?: (record: RecordSummaryLike) => boolean;
  /** 操作列宽度，默认 180。 */
  actionsColumnWidth?: number;
  /** 撤回缺失提示的语义自定义。 */
  rollbackMissingNoun?: string;
  rollbackMissingOccupied?: boolean;
  /** 是否在 row.rollbackEnabled === false 时禁用撤回按钮（仅 Mod 记录用）。 */
  enforceRollbackEnabled?: boolean;
}>();

const keyword = ref("");
const detailVisible = ref(false);
const selectedRecordIds = ref<string[]>([]);

const fullColumns = computed<VirtualColumn[]>(() => [
  ...props.listColumns,
  {
    key: "__actions",
    label: "操作",
    width: props.actionsColumnWidth ?? 180,
    slotName: "rowActionsCell"
  }
]);

const filtered = computed(() => {
  const kw = keyword.value.trim().toLowerCase();
  const extra = props.extraFilter;
  const fields = props.searchableFields ?? defaultSearchableFields;
  return props.store.records.filter((r) => {
    if (extra && !extra(r)) return false;
    if (!kw) return true;
    return fields(r).toLowerCase().includes(kw);
  });
});

function defaultSearchableFields(r: RecordSummaryLike) {
  return `${r.recordName} ${r.recordId}`;
}

function onSelectionChange(rows: unknown[]) {
  selectedRecordIds.value = (rows as RecordSummaryLike[]).map((x) => x.recordId);
}

async function openDetail(recordId: string) {
  await props.store.loadDetail(recordId);
  detailVisible.value = true;
}

async function rollbackRecord(record: RecordSummaryLike) {
  if (props.enforceRollbackEnabled && !record.rollbackEnabled) {
    ElMessage.warning("此记录创建时未启用回滚，无法撤回");
    return;
  }
  const check = await props.store.checkRollback(record.recordId);
  await confirmMissingPaths(check.missingPaths.length, {
    noun: props.rollbackMissingNoun,
    occupied: props.rollbackMissingOccupied
  });
  const r = await props.store.rollback(record.recordId, null, true);
  ElMessage.success(formatRollbackToast(r));
  await props.store.refreshRecords();
  if (props.store.currentDetail?.summary.recordId === record.recordId) {
    await props.store.loadDetail(record.recordId);
  }
}

async function deleteSelected() {
  if (!selectedRecordIds.value.length) {
    ElMessage.warning("请先勾选要删除的记录");
    return;
  }
  await ElMessageBox.confirm(
    `确认删除选中的 ${selectedRecordIds.value.length} 条记录？`,
    "删除确认",
    { type: "warning" }
  );
  await props.store.removeBatch(selectedRecordIds.value);
  selectedRecordIds.value = [];
  ElMessage.success("删除完成");
}

defineExpose({ openDetail, rollbackRecord });
</script>

<template>
  <Panel class="record-tab" :padded="false" compact>
    <template #header>
      <span class="panel-title">{{ panelTitle }}</span>
      <slot name="headerExtra" />
      <el-input
        v-model="keyword"
        clearable
        :placeholder="searchPlaceholder"
        size="small"
        class="header-search"
      />
    </template>
    <template #actions>
      <el-button
        type="danger"
        size="small"
        :disabled="!selectedRecordIds.length"
        @click="deleteSelected"
      >
        删除选中（{{ selectedRecordIds.length }}）
      </el-button>
    </template>

    <VirtualTable
      :rows="filtered"
      :columns="fullColumns"
      :item-height="40"
      row-key="recordId"
      :column-config-key="listColumnConfigKey"
      class="records-table"
      selectable
      fit-width
      @selectionChange="onSelectionChange"
    >
      <template #rowActionsCell="{ row }">
        <el-button size="small" @click="openDetail((row as RecordSummaryLike).recordId)">详情</el-button>
        <slot name="rowActions" :row="row as RecordSummaryLike" />
        <el-button
          size="small"
          type="warning"
          :disabled="enforceRollbackEnabled && !(row as RecordSummaryLike).rollbackEnabled"
          :title="
            enforceRollbackEnabled && !(row as RecordSummaryLike).rollbackEnabled
              ? '此记录创建时未启用回滚'
              : ''
          "
          @click="rollbackRecord(row as RecordSummaryLike)"
        >
          撤回
        </el-button>
      </template>
    </VirtualTable>

    <el-drawer v-model="detailVisible" size="60%" :title="detailTitle" class="detail-drawer">
      <template v-if="store.currentDetail">
        <div class="detail-body">
          <el-descriptions :column="1" border size="small">
            <el-descriptions-item label="记录名">{{ store.currentDetail.summary.recordName }}</el-descriptions-item>
            <slot name="extraDescription" :summary="store.currentDetail.summary" />
            <el-descriptions-item label="状态">{{ store.currentDetail.summary.rollbackStatus }}</el-descriptions-item>
          </el-descriptions>
          <VirtualTable
            :rows="store.currentDetail.items"
            :columns="detailColumns"
            :item-height="36"
            :overscan="10"
            row-key="itemId"
            :column-config-key="detailColumnConfigKey"
            fit-width
            class="detail-table"
          />
        </div>
      </template>
    </el-drawer>
  </Panel>
</template>

<style scoped>
.record-tab {
  height: 100%;
}

.panel-title {
  font-size: var(--ff-font-lg);
  font-weight: 600;
}

.header-search {
  width: 240px;
  max-width: 40vw;
}

.records-table {
  flex: 1;
  min-height: 0;
  border: 0;
  border-radius: 0;
}

.detail-drawer :deep(.el-drawer__body) {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.detail-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
}
.detail-table {
  flex: 1;
  min-height: 0;
}
</style>
