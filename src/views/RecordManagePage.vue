<script setup lang="ts">
/**
 * 记录管理页：哈希 / 后缀 / 空文件夹 / Mod 记录共用同一套"Panel + TabBar +
 * VirtualTable"布局。
 *
 * # 布局契约
 * - 页面 root 是 flex column，两行：TabBar + TabHost。
 * - TabHost 是 relative 容器，三个子视图 absolute 铺满，用 v-show 切换（不卸载
 *   内部状态），避免 el-tabs 切 pane 时触发 VirtualTable 重新测量闪烁。
 * - 每个子视图都是独立的 Panel，header 放搜索 + 批删按钮，body 让 VirtualTable
 *   以 auto-height 模式撑满。
 */

import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useRecordStore } from "../stores/record";
import { useSuffixStore } from "../stores/suffix";
import { useEmptyDirsStore } from "../stores/emptyDirs";
import { useModToolsStore } from "../stores/modTools";
import Panel from "../components/common/Panel.vue";
import TabBar from "../components/common/TabBar.vue";
import VirtualTable from "../components/common/VirtualTable.vue";
import RecordDetailDrawer from "../components/RecordDetailDrawer.vue";
import type { HashIndexRecord } from "../types/record";
import type { VirtualColumn } from "../types/virtualTable";
import { formatTimestamp } from "../utils/format";

const recordStore = useRecordStore();
const suffixStore = useSuffixStore();
const emptyDirsStore = useEmptyDirsStore();
const modToolsStore = useModToolsStore();

const activeTab = ref<"hash" | "suffix" | "emptyDirs" | "mod">("hash");
const tabs = [
  { label: "哈希记录", value: "hash" },
  { label: "后缀修改", value: "suffix" },
  { label: "空文件夹清理", value: "emptyDirs" },
  { label: "Mod 操作", value: "mod" }
];

const keyword = ref("");
const detailVisible = ref(false);
const currentRecord = ref<HashIndexRecord | null>(null);

const suffixKeyword = ref("");
const suffixDetailVisible = ref(false);

const emptyDirsKeyword = ref("");
const emptyDirsDetailVisible = ref(false);

const modKeyword = ref("");
const modKindFilter = ref<
  "all" | "rename" | "organize" | "modify" | "duplicate_delete" | "version_delete"
>("all");
const modDetailVisible = ref(false);

const selectedHashRecordIds = ref<string[]>([]);
const selectedSuffixRecordIds = ref<string[]>([]);
const selectedEmptyDirRecordIds = ref<string[]>([]);
const selectedModRecordIds = ref<string[]>([]);

const filtered = computed(() => {
  const kw = keyword.value.trim().toLowerCase();
  if (!kw) return recordStore.list;
  return recordStore.list.filter((x) =>
    `${x.recordName} ${x.recordId}`.toLowerCase().includes(kw)
  );
});

const suffixFiltered = computed(() => {
  const kw = suffixKeyword.value.trim().toLowerCase();
  if (!kw) return suffixStore.records;
  return suffixStore.records.filter((x) =>
    `${x.recordName} ${x.recordId} ${x.targetSuffix}`.toLowerCase().includes(kw)
  );
});

const emptyDirsFiltered = computed(() => {
  const kw = emptyDirsKeyword.value.trim().toLowerCase();
  if (!kw) return emptyDirsStore.records;
  return emptyDirsStore.records.filter((x) =>
    `${x.recordName} ${x.recordId} ${x.kind}`.toLowerCase().includes(kw)
  );
});

const modFiltered = computed(() => {
  const kw = modKeyword.value.trim().toLowerCase();
  const kind = modKindFilter.value;
  return modToolsStore.records.filter((x) => {
    if (kind !== "all" && x.kind !== kind) return false;
    if (!kw) return true;
    return `${x.recordName} ${x.recordId} ${x.kind}`.toLowerCase().includes(kw);
  });
});

function onHashSelectionChange(rows: any[]) {
  selectedHashRecordIds.value = rows.map((x) => x.recordId);
}
function onSuffixSelectionChange(rows: any[]) {
  selectedSuffixRecordIds.value = rows.map((x) => x.recordId);
}
function onEmptyDirsSelectionChange(rows: any[]) {
  selectedEmptyDirRecordIds.value = rows.map((x) => x.recordId);
}
function onModSelectionChange(rows: any[]) {
  selectedModRecordIds.value = rows.map((x) => x.recordId);
}

async function openDetail(recordId: string) {
  currentRecord.value = await recordStore.detail(recordId);
  detailVisible.value = true;
}

function applyRecord(recordId: string) {
  recordStore.select(recordId);
  ElMessage.success("已设置为当前任务使用记录");
}

async function rename(row: any) {
  const { value } = await ElMessageBox.prompt("请输入新名称", "重命名", {
    inputValue: row.recordName,
    inputValidator: (v) => !!v?.trim() || "名称不能为空"
  });
  if (!value) return;
  await recordStore.rename(row.recordId, value.trim());
  ElMessage.success("重命名成功");
}

async function deleteSelectedHashRecords() {
  if (!selectedHashRecordIds.value.length) {
    ElMessage.warning("请先勾选要删除的哈希记录");
    return;
  }
  await ElMessageBox.confirm(
    `确认删除选中的 ${selectedHashRecordIds.value.length} 条哈希记录？`,
    "删除确认",
    { type: "warning" }
  );
  for (const id of selectedHashRecordIds.value) await recordStore.remove(id);
  selectedHashRecordIds.value = [];
  ElMessage.success("删除完成");
}

async function openSuffixDetail(recordId: string) {
  await suffixStore.loadDetail(recordId);
  suffixDetailVisible.value = true;
}

async function rollbackSuffixRecord(recordId: string) {
  const check = await suffixStore.checkRollback(recordId);
  if (check.missingPaths.length) {
    await ElMessageBox.confirm(
      `该记录有 ${check.missingPaths.length} 个文件不存在，仅撤回存在文件，继续？`,
      "缺失提示",
      { type: "warning" }
    );
  }
  const r = await suffixStore.rollback(recordId, null, true);
  ElMessage.success(
    `撤回完成：成功 ${r.success}，失败 ${r.failed}，跳过缺失 ${r.skippedMissing}`
  );
  await suffixStore.refreshRecords();
  if (suffixStore.currentDetail?.summary.recordId === recordId) {
    await suffixStore.loadDetail(recordId);
  }
}

async function deleteSelectedSuffixRecords() {
  if (!selectedSuffixRecordIds.value.length) {
    ElMessage.warning("请先勾选要删除的后缀记录");
    return;
  }
  await ElMessageBox.confirm(
    `确认删除选中的 ${selectedSuffixRecordIds.value.length} 条后缀记录？`,
    "删除确认",
    { type: "warning" }
  );
  await suffixStore.removeBatch(selectedSuffixRecordIds.value);
  selectedSuffixRecordIds.value = [];
  ElMessage.success("删除完成");
}

async function openEmptyDirsDetail(recordId: string) {
  await emptyDirsStore.loadDetail(recordId);
  emptyDirsDetailVisible.value = true;
}

async function rollbackEmptyDirsRecord(recordId: string) {
  const check = await emptyDirsStore.checkRollback(recordId);
  if (check.missingPaths.length) {
    await ElMessageBox.confirm(
      `该记录有 ${check.missingPaths.length} 个路径被非目录文件占用，仅恢复可创建目录，继续？`,
      "占用提示",
      { type: "warning" }
    );
  }
  const r = await emptyDirsStore.rollback(recordId, null, true);
  ElMessage.success(`撤回完成：成功 ${r.success}，失败 ${r.failed}`);
  await emptyDirsStore.refreshRecords();
  if (emptyDirsStore.currentDetail?.summary.recordId === recordId) {
    await emptyDirsStore.loadDetail(recordId);
  }
}

async function deleteSelectedEmptyDirRecords() {
  if (!selectedEmptyDirRecordIds.value.length) {
    ElMessage.warning("请先勾选要删除的空文件夹记录");
    return;
  }
  await ElMessageBox.confirm(
    `确认删除选中的 ${selectedEmptyDirRecordIds.value.length} 条空文件夹清理记录？`,
    "删除确认",
    { type: "warning" }
  );
  await emptyDirsStore.removeBatch(selectedEmptyDirRecordIds.value);
  selectedEmptyDirRecordIds.value = [];
  ElMessage.success("删除完成");
}

async function openModDetail(recordId: string) {
  await modToolsStore.loadDetail(recordId);
  modDetailVisible.value = true;
}

async function renameModRecord(row: any) {
  const { value } = await ElMessageBox.prompt("请输入新名称", "重命名", {
    inputValue: row.recordName,
    inputValidator: (v) => !!v?.trim() || "名称不能为空"
  });
  if (!value) return;
  await modToolsStore.rename(row.recordId, value.trim());
  ElMessage.success("重命名成功");
}

async function rollbackModRecord(recordId: string) {
  const check = await modToolsStore.checkRollback(recordId);
  if (check.missingPaths.length) {
    await ElMessageBox.confirm(
      `该记录有 ${check.missingPaths.length} 个文件不存在，仅撤回存在文件，继续？`,
      "缺失提示",
      { type: "warning" }
    );
  }
  const r = await modToolsStore.rollback(recordId, null, true);
  ElMessage.success(
    `撤回完成：成功 ${r.success}，失败 ${r.failed}，跳过缺失 ${r.skippedMissing}`
  );
  await modToolsStore.refreshRecords();
  if (modToolsStore.currentDetail?.summary.recordId === recordId) {
    await modToolsStore.loadDetail(recordId);
  }
}

async function deleteSelectedModRecords() {
  if (!selectedModRecordIds.value.length) {
    ElMessage.warning("请先勾选要删除的 Mod 操作记录");
    return;
  }
  await ElMessageBox.confirm(
    `确认删除选中的 ${selectedModRecordIds.value.length} 条 Mod 操作记录？`,
    "删除确认",
    { type: "warning" }
  );
  await modToolsStore.removeBatch(selectedModRecordIds.value);
  selectedModRecordIds.value = [];
  ElMessage.success("删除完成");
}

function formatModKind(kind: string) {
  if (kind === "rename") return "Mod 重命名";
  if (kind === "organize") return "文件夹归类";
  if (kind === "modify") return "移除版本限制";
  if (kind === "duplicate_delete") return "删除重复 MOD";
  if (kind === "version_delete") return "删除旧版本 MOD";
  return kind;
}

function formatEmptyDirKind(kind: string) {
  if (kind === "delete") return "删除空文件夹";
  return kind;
}

onMounted(async () => {
  await recordStore.refresh();
  await suffixStore.refreshRecords();
  await emptyDirsStore.refreshRecords();
  await modToolsStore.refreshRecords();
});

const suffixDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "旧路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "新路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "修改", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];

const modDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "旧路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "新路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "执行", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "applyError", label: "执行错误", minWidth: 180, ellipsis: true, resizable: true },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];

const emptyDirsDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "目录路径", minWidth: 320, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "删除", width: 70 },
  { key: "rollbackSuccess", label: "撤回", width: 70 },
  { key: "applyError", label: "删除错误", minWidth: 180, ellipsis: true, resizable: true },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true }
];

const hashListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 240, ellipsis: true, resizable: true },
  {
    key: "createdAt",
    label: "创建时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "entryCount", label: "条目数", width: 90 },
  { key: "__actions", label: "操作", width: 260, slotName: "hashActions" }
];

const suffixListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  { key: "targetSuffix", label: "目标后缀", width: 100 },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 },
  { key: "__actions", label: "操作", width: 180, slotName: "suffixActions" }
];

const emptyDirsListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  {
    key: "kind",
    label: "类型",
    width: 120,
    formatter: (_row, v: string) => formatEmptyDirKind(v)
  },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 },
  { key: "__actions", label: "操作", width: 180, slotName: "emptyDirsActions" }
];

const modListColumns: VirtualColumn[] = [
  { key: "recordName", label: "记录名", minWidth: 220, ellipsis: true, resizable: true },
  {
    key: "kind",
    label: "类型",
    width: 110,
    formatter: (_row, v: string) => formatModKind(v)
  },
  {
    key: "createdAt",
    label: "时间",
    width: 170,
    formatter: (_row, v: number) => formatTimestamp(v)
  },
  { key: "successItems", label: "成功", width: 70 },
  { key: "totalItems", label: "总数", width: 70 },
  { key: "rollbackStatus", label: "回滚状态", width: 130 },
  { key: "__actions", label: "操作", width: 260, slotName: "modActions" }
];
</script>

<template>
  <div class="records-page">
    <div class="records-header">
      <TabBar
        :model-value="activeTab"
        :items="tabs"
        @update:model-value="(v: string) => (activeTab = v as 'hash' | 'suffix' | 'emptyDirs' | 'mod')"
      />
    </div>

    <div class="records-host">
      <!-- 哈希记录 -->
      <Panel v-show="activeTab === 'hash'" class="records-panel" :padded="false" compact>
        <template #header>
          <span class="panel-title">哈希记录管理</span>
          <el-input
            v-model="keyword"
            clearable
            placeholder="搜索记录…"
            size="small"
            class="header-search"
          />
        </template>
        <template #actions>
          <el-button type="danger" size="small" :disabled="!selectedHashRecordIds.length" @click="deleteSelectedHashRecords">
            删除选中（{{ selectedHashRecordIds.length }}）
          </el-button>
        </template>

        <VirtualTable
          :rows="filtered"
          :columns="hashListColumns"
          :item-height="40"
          row-key="recordId"
          column-config-key="records:hash-list"
          class="records-table"
          selectable
          fit-width
          @selectionChange="onHashSelectionChange"
        >
          <template #hashActions="{ row }">
            <el-button size="small" @click="openDetail(row.recordId)">详情</el-button>
            <el-button size="small" type="primary" @click="applyRecord(row.recordId)">应用</el-button>
            <el-button size="small" @click="rename(row)">重命名</el-button>
          </template>
        </VirtualTable>
      </Panel>

      <!-- 后缀记录 -->
      <Panel v-show="activeTab === 'suffix'" class="records-panel" :padded="false" compact>
        <template #header>
          <span class="panel-title">后缀修改记录</span>
          <el-input
            v-model="suffixKeyword"
            clearable
            placeholder="搜索后缀记录…"
            size="small"
            class="header-search"
          />
        </template>
        <template #actions>
          <el-button type="danger" size="small" :disabled="!selectedSuffixRecordIds.length" @click="deleteSelectedSuffixRecords">
            删除选中（{{ selectedSuffixRecordIds.length }}）
          </el-button>
        </template>

        <VirtualTable
          :rows="suffixFiltered"
          :columns="suffixListColumns"
          :item-height="40"
          row-key="recordId"
          column-config-key="records:suffix-list"
          class="records-table"
          selectable
          fit-width
          @selectionChange="onSuffixSelectionChange"
        >
          <template #suffixActions="{ row }">
            <el-button size="small" @click="openSuffixDetail(row.recordId)">详情</el-button>
            <el-button size="small" type="warning" @click="rollbackSuffixRecord(row.recordId)">撤回</el-button>
          </template>
        </VirtualTable>

        <el-drawer v-model="suffixDetailVisible" size="60%" title="后缀记录详情" class="detail-drawer">
          <template v-if="suffixStore.currentDetail">
            <div class="detail-body">
              <el-descriptions :column="1" border size="small">
                <el-descriptions-item label="记录名">{{ suffixStore.currentDetail.summary.recordName }}</el-descriptions-item>
                <el-descriptions-item label="目标后缀">{{ suffixStore.currentDetail.summary.targetSuffix }}</el-descriptions-item>
                <el-descriptions-item label="状态">{{ suffixStore.currentDetail.summary.rollbackStatus }}</el-descriptions-item>
              </el-descriptions>
              <VirtualTable
                :rows="suffixStore.currentDetail.items"
                :columns="suffixDetailColumns"
                :item-height="36"
                :overscan="10"
                row-key="itemId"
                column-config-key="records:suffix-detail"
                fit-width
                class="detail-table"
              />
            </div>
          </template>
        </el-drawer>
      </Panel>

      <!-- 空文件夹清理记录 -->
      <Panel v-show="activeTab === 'emptyDirs'" class="records-panel" :padded="false" compact>
        <template #header>
          <span class="panel-title">空文件夹清理记录</span>
          <el-input
            v-model="emptyDirsKeyword"
            clearable
            placeholder="搜索清理记录…"
            size="small"
            class="header-search"
          />
        </template>
        <template #actions>
          <el-button type="danger" size="small" :disabled="!selectedEmptyDirRecordIds.length" @click="deleteSelectedEmptyDirRecords">
            删除选中（{{ selectedEmptyDirRecordIds.length }}）
          </el-button>
        </template>

        <VirtualTable
          :rows="emptyDirsFiltered"
          :columns="emptyDirsListColumns"
          :item-height="40"
          row-key="recordId"
          column-config-key="records:empty-dirs-list"
          class="records-table"
          selectable
          fit-width
          @selectionChange="onEmptyDirsSelectionChange"
        >
          <template #emptyDirsActions="{ row }">
            <el-button size="small" @click="openEmptyDirsDetail(row.recordId)">详情</el-button>
            <el-button size="small" type="warning" @click="rollbackEmptyDirsRecord(row.recordId)">撤回</el-button>
          </template>
        </VirtualTable>

        <el-drawer v-model="emptyDirsDetailVisible" size="60%" title="空文件夹清理详情" class="detail-drawer">
          <template v-if="emptyDirsStore.currentDetail">
            <div class="detail-body">
              <el-descriptions :column="1" border size="small">
                <el-descriptions-item label="记录名">{{ emptyDirsStore.currentDetail.summary.recordName }}</el-descriptions-item>
                <el-descriptions-item label="类型">{{ formatEmptyDirKind(emptyDirsStore.currentDetail.summary.kind) }}</el-descriptions-item>
                <el-descriptions-item label="状态">{{ emptyDirsStore.currentDetail.summary.rollbackStatus }}</el-descriptions-item>
              </el-descriptions>
              <VirtualTable
                :rows="emptyDirsStore.currentDetail.items"
                :columns="emptyDirsDetailColumns"
                :item-height="36"
                :overscan="10"
                row-key="itemId"
                column-config-key="records:empty-dirs-detail"
                fit-width
                class="detail-table"
              />
            </div>
          </template>
        </el-drawer>
      </Panel>

      <!-- Mod 记录 -->
      <Panel v-show="activeTab === 'mod'" class="records-panel" :padded="false" compact>
        <template #header>
          <span class="panel-title">Mod 操作记录</span>
          <el-radio-group v-model="modKindFilter" size="small" class="kind-filter">
            <el-radio-button value="all">全部</el-radio-button>
            <el-radio-button value="rename">重命名</el-radio-button>
            <el-radio-button value="organize">归类</el-radio-button>
            <el-radio-button value="modify">版本修改</el-radio-button>
            <el-radio-button value="duplicate_delete">重复删除</el-radio-button>
            <el-radio-button value="version_delete">旧版删除</el-radio-button>
          </el-radio-group>
          <el-input
            v-model="modKeyword"
            clearable
            placeholder="搜索记录…"
            size="small"
            class="header-search"
          />
        </template>
        <template #actions>
          <el-button type="danger" size="small" :disabled="!selectedModRecordIds.length" @click="deleteSelectedModRecords">
            删除选中（{{ selectedModRecordIds.length }}）
          </el-button>
        </template>

        <VirtualTable
          :rows="modFiltered"
          :columns="modListColumns"
          :item-height="40"
          row-key="recordId"
          column-config-key="records:mod-list"
          class="records-table"
          selectable
          fit-width
          @selectionChange="onModSelectionChange"
        >
          <template #modActions="{ row }">
            <el-button size="small" @click="openModDetail(row.recordId)">详情</el-button>
            <el-button size="small" @click="renameModRecord(row)">重命名</el-button>
            <el-button size="small" type="warning" @click="rollbackModRecord(row.recordId)">撤回</el-button>
          </template>
        </VirtualTable>

        <el-drawer v-model="modDetailVisible" size="60%" title="Mod 操作记录详情" class="detail-drawer">
          <template v-if="modToolsStore.currentDetail">
            <div class="detail-body">
              <el-descriptions :column="1" border size="small">
                <el-descriptions-item label="记录名">{{ modToolsStore.currentDetail.summary.recordName }}</el-descriptions-item>
                <el-descriptions-item label="类型">{{ formatModKind(modToolsStore.currentDetail.summary.kind) }}</el-descriptions-item>
                <el-descriptions-item label="状态">{{ modToolsStore.currentDetail.summary.rollbackStatus }}</el-descriptions-item>
              </el-descriptions>
              <VirtualTable
                :rows="modToolsStore.currentDetail.items"
                :columns="modDetailColumns"
                :item-height="36"
                :overscan="10"
                row-key="itemId"
                column-config-key="records:mod-detail"
                fit-width
                class="detail-table"
              />
            </div>
          </template>
        </el-drawer>
      </Panel>
    </div>

    <RecordDetailDrawer v-model="detailVisible" :record="currentRecord" />
  </div>
</template>

<style scoped>
.records-page {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
}

.records-header {
  flex-shrink: 0;
}

.records-host {
  flex: 1;
  min-height: 0;
  position: relative;
}
.records-host > * {
  position: absolute;
  inset: 0;
}

.records-panel {
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

.kind-filter {
  flex-shrink: 0;
}

.records-table {
  flex: 1;
  min-height: 0;
  border: 0;
  border-radius: 0;
}

/* ---- 详情抽屉 ---- */
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
