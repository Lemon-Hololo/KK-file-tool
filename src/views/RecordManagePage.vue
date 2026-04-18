<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useRecordStore } from "../stores/record";
import { useSuffixStore } from "../stores/suffix";
import { useModToolsStore } from "../stores/modTools";
import RecordDetailDrawer from "../components/RecordDetailDrawer.vue";
import VirtualTable from "../components/common/VirtualTable.vue";
import type { HashIndexRecord } from "../types/record";
import type { VirtualColumn } from "../types/virtualTable";

const recordStore = useRecordStore();
const suffixStore = useSuffixStore();
const modToolsStore = useModToolsStore();

const keyword = ref("");
const detailVisible = ref(false);
const currentRecord = ref<HashIndexRecord | null>(null);

const suffixKeyword = ref("");
const suffixDetailVisible = ref(false);

const modKeyword = ref("");
const modKindFilter = ref<"all" | "rename" | "organize">("all");
const modDetailVisible = ref(false);

// 选择集合（统一单删/批删）
const selectedHashRecordIds = ref<string[]>([]);
const selectedSuffixRecordIds = ref<string[]>([]);
const selectedModRecordIds = ref<string[]>([]);

const filtered = computed(() => {
  const kw = keyword.value.trim().toLowerCase();
  if (!kw) return recordStore.list;
  return recordStore.list.filter((x) => `${x.recordName} ${x.recordId}`.toLowerCase().includes(kw));
});

const suffixFiltered = computed(() => {
  const kw = suffixKeyword.value.trim().toLowerCase();
  if (!kw) return suffixStore.records;
  return suffixStore.records.filter((x) =>
    `${x.recordName} ${x.recordId} ${x.targetSuffix}`.toLowerCase().includes(kw)
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

  for (const id of selectedHashRecordIds.value) {
    await recordStore.remove(id);
  }
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
  ElMessage.success(`撤回完成：成功 ${r.success}，失败 ${r.failed}，跳过缺失 ${r.skippedMissing}`);
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
  ElMessage.success(`撤回完成：成功 ${r.success}，失败 ${r.failed}，跳过缺失 ${r.skippedMissing}`);
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
  return kind;
}

onMounted(async () => {
  await recordStore.refresh();
  await suffixStore.refreshRecords();
  await modToolsStore.refreshRecords();
});

const suffixDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "旧路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "新路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "修改成功", width: 90 },
  { key: "rollbackSuccess", label: "撤回成功", width: 90 },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true },
];

const modDetailColumns: VirtualColumn[] = [
  { key: "oldPath", label: "旧路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "newPath", label: "新路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "applySuccess", label: "执行成功", width: 90 },
  { key: "rollbackSuccess", label: "撤回成功", width: 90 },
  { key: "applyError", label: "执行错误", minWidth: 180, ellipsis: true, resizable: true },
  { key: "rollbackError", label: "撤回错误", minWidth: 180, ellipsis: true, resizable: true },
];
</script>

<template>
  <el-tabs>
    <el-tab-pane label="哈希记录管理">
      <el-card>
        <template #header>
          <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;">
            <span>记录管理</span>
            <el-space>
              <el-input v-model="keyword" clearable placeholder="搜索记录..." style="width:240px" />
              <el-button type="danger" @click="deleteSelectedHashRecords">
                删除选中（{{ selectedHashRecordIds.length }}）
              </el-button>
            </el-space>
          </div>
        </template>

        <el-table :data="filtered" border @selection-change="onHashSelectionChange">
          <el-table-column type="selection" width="55" />
          <el-table-column prop="recordName" label="记录名" min-width="240" />
          <el-table-column prop="createdAt" label="创建时间戳" width="170" />
          <el-table-column prop="entryCount" label="条目数" width="100" />
          <el-table-column label="操作" width="260">
            <template #default="{ row }">
              <el-button size="small" @click="openDetail(row.recordId)">详情</el-button>
              <el-button size="small" type="primary" @click="applyRecord(row.recordId)">应用</el-button>
              <el-button size="small" @click="rename(row)">重命名</el-button>
            </template>
          </el-table-column>
        </el-table>
      </el-card>
    </el-tab-pane>

    <el-tab-pane label="后缀修改记录">
      <el-card>
        <template #header>
          <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;">
            <span>后缀修改记录</span>
            <el-space>
              <el-input v-model="suffixKeyword" clearable placeholder="搜索后缀记录..." style="width:240px" />
              <el-button type="danger" @click="deleteSelectedSuffixRecords">
                删除选中（{{ selectedSuffixRecordIds.length }}）
              </el-button>
            </el-space>
          </div>
        </template>

        <el-table :data="suffixFiltered" border @selection-change="onSuffixSelectionChange">
          <el-table-column type="selection" width="55" />
          <el-table-column prop="recordName" label="记录名" min-width="220" />
          <el-table-column prop="targetSuffix" label="目标后缀" width="110" />
          <el-table-column prop="createdAt" label="时间戳" width="170" />
          <el-table-column prop="successItems" label="成功" width="80" />
          <el-table-column prop="totalItems" label="总数" width="80" />
          <el-table-column prop="rollbackStatus" label="回滚状态" width="150" />
          <el-table-column label="操作" width="180">
            <template #default="{ row }">
              <el-button size="small" @click="openSuffixDetail(row.recordId)">详情</el-button>
              <el-button size="small" type="warning" @click="rollbackSuffixRecord(row.recordId)">撤回</el-button>
            </template>
          </el-table-column>
        </el-table>
      </el-card>

      <el-drawer v-model="suffixDetailVisible" size="60%" title="后缀记录详情">
        <template v-if="suffixStore.currentDetail">
          <el-descriptions :column="1" border>
            <el-descriptions-item label="记录名">{{ suffixStore.currentDetail.summary.recordName }}</el-descriptions-item>
            <el-descriptions-item label="目标后缀">{{ suffixStore.currentDetail.summary.targetSuffix }}</el-descriptions-item>
            <el-descriptions-item label="状态">{{ suffixStore.currentDetail.summary.rollbackStatus }}</el-descriptions-item>
          </el-descriptions>

          <VirtualTable
            :rows="suffixStore.currentDetail.items"
            :columns="suffixDetailColumns"
            :height="480"
            :item-height="36"
            :overscan="10"
            row-key="itemId"
            style="margin-top:12px"
          />
        </template>
      </el-drawer>
    </el-tab-pane>

    <el-tab-pane label="Mod 操作记录">
      <el-card>
        <template #header>
          <div style="display:flex;justify-content:space-between;align-items:center;gap:8px;flex-wrap:wrap;">
            <span>Mod 操作记录</span>
            <el-space>
              <el-radio-group v-model="modKindFilter" size="small">
                <el-radio-button value="all">全部</el-radio-button>
                <el-radio-button value="rename">重命名</el-radio-button>
                <el-radio-button value="organize">归类</el-radio-button>
              </el-radio-group>
              <el-input v-model="modKeyword" clearable placeholder="搜索记录..." style="width:240px" />
              <el-button type="danger" @click="deleteSelectedModRecords">
                删除选中（{{ selectedModRecordIds.length }}）
              </el-button>
            </el-space>
          </div>
        </template>

        <el-table :data="modFiltered" border @selection-change="onModSelectionChange">
          <el-table-column type="selection" width="55" />
          <el-table-column prop="recordName" label="记录名" min-width="220" />
          <el-table-column label="类型" width="120">
            <template #default="{ row }">{{ formatModKind(row.kind) }}</template>
          </el-table-column>
          <el-table-column prop="createdAt" label="时间戳" width="170" />
          <el-table-column prop="successItems" label="成功" width="80" />
          <el-table-column prop="totalItems" label="总数" width="80" />
          <el-table-column prop="rollbackStatus" label="回滚状态" width="150" />
          <el-table-column label="操作" width="260">
            <template #default="{ row }">
              <el-button size="small" @click="openModDetail(row.recordId)">详情</el-button>
              <el-button size="small" @click="renameModRecord(row)">重命名</el-button>
              <el-button size="small" type="warning" @click="rollbackModRecord(row.recordId)">撤回</el-button>
            </template>
          </el-table-column>
        </el-table>
      </el-card>

      <el-drawer v-model="modDetailVisible" size="60%" title="Mod 操作记录详情">
        <template v-if="modToolsStore.currentDetail">
          <el-descriptions :column="1" border>
            <el-descriptions-item label="记录名">{{ modToolsStore.currentDetail.summary.recordName }}</el-descriptions-item>
            <el-descriptions-item label="类型">{{ formatModKind(modToolsStore.currentDetail.summary.kind) }}</el-descriptions-item>
            <el-descriptions-item label="状态">{{ modToolsStore.currentDetail.summary.rollbackStatus }}</el-descriptions-item>
          </el-descriptions>

          <VirtualTable
            :rows="modToolsStore.currentDetail.items"
            :columns="modDetailColumns"
            :height="480"
            :item-height="36"
            :overscan="10"
            row-key="itemId"
            style="margin-top:12px"
          />
        </template>
      </el-drawer>
    </el-tab-pane>
  </el-tabs>

  <RecordDetailDrawer v-model="detailVisible" :record="currentRecord" />
</template>
