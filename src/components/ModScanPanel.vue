<script setup lang="ts">
/**
 * Mod 版本限制扫描面板。
 *
 * 扫描 `.zipmod` 内 `manifest.xml` 中含指定 `<game>` 关键字的文件，
 * 并允许对勾选的条目应用"移除版本限制"操作（写入 Mod 操作记录，可在
 * 记录管理页撤回）。
 *
 * 布局：自有 Panel 包壳（padded=true），顶部工具条 + 状态提示 + VirtualTable
 * (auto-height)。不再嵌 el-card + :deep 覆写。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useStorage } from "@vueuse/core";

import { useModToolsStore } from "../stores/modTools";
import { useConfigStore } from "../stores/config";
import { revealInExplorer } from "../services/task";
import { DEFAULT_EXTREME_ROW_THRESHOLD, EXTREME_OVERSCAN, NORMAL_OVERSCAN } from "../constants/task";
import { stripWindowsExtendedPrefix } from "../utils/path";
import type { ModScanMatch } from "../types/modTools";
import type { VirtualColumn } from "../types/virtualTable";
import Panel from "./common/Panel.vue";
import PreviewPanel from "./PreviewPanel.vue";
import VirtualTable from "./common/VirtualTable.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useModToolsStore();
const configStore = useConfigStore();

const keyword = useStorage<string>(
  "modScanKeyword",
  configStore.settings.modScanDefaultKeyword || "Koikatsu"
);

const selectedRows = ref<ModScanMatch[]>([]);
const applying = ref(false);
const panelBusy = computed(() => store.scan.running || applying.value || store.busy.modify);

const tableData = computed<ModScanMatch[]>(() => store.scan.matches);
const isExtreme = computed(() => {
  const th = configStore.settings.extremeRowThreshold || DEFAULT_EXTREME_ROW_THRESHOLD;
  return tableData.value.length > th;
});

const columns = computed<VirtualColumn[]>(() => [
  { key: "filePath", label: "文件", minWidth: 360, ellipsis: true, resizable: true, slotName: "filePath" },
  { key: "author", label: "作者", width: 140, ellipsis: true, resizable: true },
  { key: "guid", label: "GUID", minWidth: 200, ellipsis: true, resizable: true },
  { key: "version", label: "版本", width: 120, resizable: true },
  { key: "matchedKeyword", label: "命中关键字", width: 140, resizable: true }
]);

const statusText = computed(() => {
  if (store.scan.running) return "扫描中…";
  if (!store.scan.taskId) return "尚未开始";
  const s = store.scan;
  return `已完成（匹配 ${s.matches.length}，扫描 ${s.totalScanned}，错误 ${s.totalErrors}${s.cancelled ? "，已取消" : ""}）`;
});

const statusClass = computed(() =>
  store.scan.running ? "is-running" : store.scan.taskId ? "is-done" : "is-idle"
);

async function startScan() {
  if (!keyword.value.trim()) {
    ElMessage.warning("请输入关键字");
    return;
  }
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;

  try {
    selectedRows.value = [];
    await store.startScan(normalized, keyword.value.trim());
    ElMessage.success("扫描已开始");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function stopScan() {
  try {
    await store.stopScan();
    ElMessage.info("已请求停止");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function openFolder(filePath: string) {
  await revealInExplorer(filePath);
}

function onSelectionChange(rows: unknown[]) {
  selectedRows.value = rows as ModScanMatch[];
}

/**
 * 对勾选的 `.zipmod` 应用"移除 `<game>KEYWORD</game>`"。
 *
 * 修改是就地重写 zip（备份保留在 `{原文件}.kk-file-tool-bak-...`），
 * 提交即写入可撤回的 Mod 操作记录。
 */
async function applyModifyVersion() {
  if (!selectedRows.value.length) {
    ElMessage.warning("请先勾选要修改的文件");
    return;
  }
  const kw = keyword.value.trim();
  if (!kw) {
    ElMessage.warning("关键字不能为空");
    return;
  }

  try {
    await ElMessageBox.confirm(
      `将对选中的 ${selectedRows.value.length} 个文件的 manifest.xml 移除 <game>${kw}</game> 标签，原文件会被备份并支持撤回。是否继续？`,
      "确认修改",
      { confirmButtonText: "修改", cancelButtonText: "取消", type: "warning" }
    );
  } catch {
    return;
  }

  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;

  applying.value = true;
  try {
    const filePaths = selectedRows.value.map((r) => r.filePath);
    const result = await store.applyModifyVersion(normalized, kw, filePaths);
    ElMessage.success(
      `修改完成：成功 ${result.success}，失败 ${result.failed}（记录已写入"记录管理"）`
    );
    selectedRows.value = [];
  } catch (e) {
    ElMessage.error(String(e));
  } finally {
    applying.value = false;
  }
}

onMounted(() => {
  store.initScanEvents();
});

onBeforeUnmount(() => {
  // unlisten 交由 store 管理；切换 Tab 不强制取消
});
</script>

<template>
  <Panel class="mod-scan-panel" :padded="true">
    <div class="scan-toolbar">
      <label class="inline-field">
        <span class="field-label">关键字</span>
        <el-input
          v-model="keyword"
          :disabled="panelBusy"
          placeholder="manifest.xml 中 <game> 标签内容"
          class="keyword-input"
        />
      </label>
      <div class="scan-actions">
        <el-button type="primary" :disabled="panelBusy" @click="startScan">开始扫描</el-button>
        <el-button type="warning" plain :disabled="!store.scan.running" @click="stopScan">停止</el-button>
        <el-button
          type="danger"
          :disabled="!selectedRows.length || panelBusy"
          :loading="applying"
          @click="applyModifyVersion"
        >
          修改选中（{{ selectedRows.length }}）
        </el-button>
      </div>
    </div>

    <div class="status-bar" :class="statusClass">
      <span class="status-dot" />
      <span>{{ statusText }}</span>
    </div>

    <div v-if="isExtreme" class="ops-warn">
      <span class="dot dot-warn" />
      <span>匹配结果较多，已启用极限性能模式</span>
    </div>

    <VirtualTable
      class="scan-table"
      :rows="tableData"
      :columns="columns"
      :item-height="36"
      :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
      row-key="filePath"
      column-config-key="task:mod-scan"
      fit-width
      selectable
      @selection-change="onSelectionChange"
    >
      <template #filePath="{ row }">
        <PreviewPanel :path="row.filePath">
          <button
            type="button"
            class="path-link"
            :disabled="panelBusy"
            :title="stripWindowsExtendedPrefix(row.filePath)"
            @click.stop="openFolder(row.filePath)"
          >
            {{ stripWindowsExtendedPrefix(row.filePath) }}
          </button>
        </PreviewPanel>
      </template>
    </VirtualTable>
  </Panel>
</template>

<style scoped>
.mod-scan-panel {
  height: 100%;
  min-height: 0;
}

.scan-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2) var(--ff-space-3);
  flex-shrink: 0;
}
.scan-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2);
  margin-left: auto;
}
.inline-field {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.field-label {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  font-weight: 500;
}
.keyword-input {
  width: 280px;
}

.status-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-bg-muted);
  color: var(--ff-text-secondary);
  font-size: var(--ff-font-sm);
  flex-shrink: 0;
}
.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--ff-text-muted);
}
.status-bar.is-running .status-dot {
  background: var(--ff-accent);
  animation: pulse 1.2s ease-in-out infinite;
}
.status-bar.is-done .status-dot {
  background: var(--ff-success);
}
@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(1.3); }
}

.ops-warn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-warning-soft);
  color: var(--ff-warning);
  font-size: var(--ff-font-sm);
  flex-shrink: 0;
}
.dot-warn {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ff-warning);
  flex-shrink: 0;
}

.scan-table {
  flex: 1;
  min-height: 0;
}

.path-link {
  display: block;
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  color: var(--ff-accent);
  cursor: pointer;
  font: inherit;
}

.path-link:hover {
  text-decoration: underline;
}

.path-link:disabled {
  color: var(--ff-text-muted);
  cursor: not-allowed;
  text-decoration: none;
}
</style>
