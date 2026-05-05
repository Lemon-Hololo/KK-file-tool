<script setup lang="ts">
/**
 * 记录管理页：哈希 / 后缀 / 空文件夹 / Mod 记录共用同一套"Panel + TabBar +
 * VirtualTable"布局。
 *
 * # 布局契约
 * - 页面 root 是 flex column，两行：TabBar + TabHost。
 * - TabHost 是 relative 容器，四个子视图 absolute 铺满，用 v-show 切换（不卸载
 *   内部状态），避免 el-tabs 切 pane 时触发 VirtualTable 重新测量闪烁。
 * - 后缀 / 空文件夹 / Mod 三个 tab 都走 [`RecordTab`](../components/RecordTab.vue) 组件，
 *   差异通过 props 注入；哈希 tab 因为有"应用记录"和"重命名"两个特殊操作
 *   保留单独模板。
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
import RecordTab from "../components/RecordTab.vue";
import type { HashIndexRecord } from "../types/record";
import {
  emptyDirsDetailColumns,
  emptyDirsListColumns,
  formatEmptyDirKind,
  formatModKind,
  hashListColumns,
  modDetailColumns,
  modListColumns,
  suffixDetailColumns,
  suffixListColumns
} from "../constants/recordColumns";

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

// ---- 哈希 tab 专属（与其他三个不同：多了"应用记录"和"重命名"按钮） ----
const hashKeyword = ref("");
const hashDetailVisible = ref(false);
const currentHashRecord = ref<HashIndexRecord | null>(null);
const selectedHashRecordIds = ref<string[]>([]);

const filteredHash = computed(() => {
  const kw = hashKeyword.value.trim().toLowerCase();
  if (!kw) return recordStore.list;
  return recordStore.list.filter((x) =>
    `${x.recordName} ${x.recordId}`.toLowerCase().includes(kw)
  );
});

// ---- Mod tab 专属：kind 过滤 ----
const modKindFilter = ref<
  "all" | "rename" | "organize" | "modify" | "duplicate_delete" | "version_delete"
>("all");

function modExtraFilter(record: { recordId: string; recordName: string; rollbackEnabled: boolean; [k: string]: unknown }): boolean {
  if (modKindFilter.value === "all") return true;
  return record.kind === modKindFilter.value;
}

function modSearchableFields(r: { recordName: string; recordId: string; [k: string]: unknown }): string {
  return `${r.recordName} ${r.recordId} ${String(r.kind ?? "")}`;
}

function suffixSearchableFields(r: { recordName: string; recordId: string; [k: string]: unknown }): string {
  return `${r.recordName} ${r.recordId} ${r.targetSuffix ?? ""}`;
}

function emptyDirsSearchableFields(r: { recordName: string; recordId: string; [k: string]: unknown }): string {
  return `${r.recordName} ${r.recordId} ${String(r.kind ?? "")}`;
}

// ---- 哈希 tab 操作 ----
function onHashSelectionChange(rows: any[]) {
  selectedHashRecordIds.value = rows.map((x) => x.recordId);
}

async function openHashDetail(recordId: string) {
  currentHashRecord.value = await recordStore.detail(recordId);
  hashDetailVisible.value = true;
}

function applyHashRecord(recordId: string) {
  recordStore.select(recordId);
  ElMessage.success("已设置为当前任务使用记录");
}

async function renameHashRecord(row: any) {
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

// ---- Mod tab：rowActions slot 里的"重命名"按钮 ----
async function renameModRecord(row: any) {
  const { value } = await ElMessageBox.prompt("请输入新名称", "重命名", {
    inputValue: row.recordName,
    inputValidator: (v) => !!v?.trim() || "名称不能为空"
  });
  if (!value) return;
  await modToolsStore.rename(row.recordId, value.trim());
  ElMessage.success("重命名成功");
}

onMounted(async () => {
  await Promise.all([
    recordStore.refresh(),
    suffixStore.refreshRecords(),
    emptyDirsStore.refreshRecords(),
    modToolsStore.refreshRecords()
  ]);
});
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
      <!-- 哈希记录：保留单独模板，因为有"应用记录"和"重命名"特殊操作 -->
      <Panel v-show="activeTab === 'hash'" class="records-panel" :padded="false" compact>
        <template #header>
          <span class="panel-title">哈希记录管理</span>
          <el-input
            v-model="hashKeyword"
            clearable
            placeholder="搜索记录…"
            size="small"
            class="header-search"
          />
        </template>
        <template #actions>
          <el-button
            type="danger"
            size="small"
            :disabled="!selectedHashRecordIds.length"
            @click="deleteSelectedHashRecords"
          >
            删除选中（{{ selectedHashRecordIds.length }}）
          </el-button>
        </template>

        <VirtualTable
          :rows="filteredHash"
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
            <el-button size="small" @click="openHashDetail(row.recordId)">详情</el-button>
            <el-button size="small" type="primary" @click="applyHashRecord(row.recordId)">应用</el-button>
            <el-button size="small" @click="renameHashRecord(row)">重命名</el-button>
          </template>
        </VirtualTable>
      </Panel>

      <!-- 后缀记录 -->
      <RecordTab
        v-show="activeTab === 'suffix'"
        :store="suffixStore as any"
        :list-columns="suffixListColumns"
        :detail-columns="suffixDetailColumns"
        panel-title="后缀修改记录"
        detail-title="后缀记录详情"
        search-placeholder="搜索后缀记录…"
        list-column-config-key="records:suffix-list"
        detail-column-config-key="records:suffix-detail"
        :searchable-fields="suffixSearchableFields"
      >
        <template #extraDescription="{ summary }">
          <el-descriptions-item label="目标后缀">{{ (summary as any).targetSuffix }}</el-descriptions-item>
        </template>
      </RecordTab>

      <!-- 空文件夹清理记录 -->
      <RecordTab
        v-show="activeTab === 'emptyDirs'"
        :store="emptyDirsStore as any"
        :list-columns="emptyDirsListColumns"
        :detail-columns="emptyDirsDetailColumns"
        panel-title="空文件夹清理记录"
        detail-title="空文件夹清理详情"
        search-placeholder="搜索清理记录…"
        list-column-config-key="records:empty-dirs-list"
        detail-column-config-key="records:empty-dirs-detail"
        :searchable-fields="emptyDirsSearchableFields"
        rollback-missing-noun="路径"
        rollback-missing-occupied
      >
        <template #extraDescription="{ summary }">
          <el-descriptions-item label="类型">{{ formatEmptyDirKind((summary as any).kind) }}</el-descriptions-item>
        </template>
      </RecordTab>

      <!-- Mod 记录 -->
      <RecordTab
        v-show="activeTab === 'mod'"
        :store="modToolsStore as any"
        :list-columns="modListColumns"
        :detail-columns="modDetailColumns"
        panel-title="Mod 操作记录"
        detail-title="Mod 操作记录详情"
        search-placeholder="搜索记录…"
        list-column-config-key="records:mod-list"
        detail-column-config-key="records:mod-detail"
        :searchable-fields="modSearchableFields"
        :extra-filter="modExtraFilter"
        :actions-column-width="240"
        enforce-rollback-enabled
      >
        <template #headerExtra>
          <el-radio-group v-model="modKindFilter" size="small" class="kind-filter">
            <el-radio-button value="all">全部</el-radio-button>
            <el-radio-button value="rename">重命名</el-radio-button>
            <el-radio-button value="organize">归类</el-radio-button>
            <el-radio-button value="modify">版本修改</el-radio-button>
            <el-radio-button value="duplicate_delete">重复删除</el-radio-button>
            <el-radio-button value="version_delete">旧版删除</el-radio-button>
          </el-radio-group>
        </template>
        <template #rowActions="{ row }">
          <el-button size="small" @click="renameModRecord(row)">重命名</el-button>
        </template>
        <template #extraDescription="{ summary }">
          <el-descriptions-item label="类型">{{ formatModKind((summary as any).kind) }}</el-descriptions-item>
        </template>
      </RecordTab>
    </div>

    <RecordDetailDrawer v-model="hashDetailVisible" :record="currentHashRecord" />
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
</style>
