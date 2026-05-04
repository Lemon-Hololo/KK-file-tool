<script setup lang="ts">
/**
 * 不同版本 MOD 检查面板。
 *
 * 分组规则：`guid + author` 相同但存在多个 version。默认保留最高版本，
 * 删除动作写入可撤回 Mod 操作记录。
 */
import { computed, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";

import { useModToolsStore } from "../stores/modTools";
import { useConfigStore } from "../stores/config";
import { useRuntimeStore } from "../stores/runtime";
import { revealInExplorer } from "../services/task";
import type { ModIdentityFile, ModVersionGroup } from "../types/modTools";
import { copyText } from "../utils/clipboard";
import { formatBytes, formatTimestamp } from "../utils/format";
import { stripWindowsExtendedPrefix } from "../utils/path";
import Panel from "./common/Panel.vue";
import PreviewPanel from "./PreviewPanel.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useModToolsStore();
const configStore = useConfigStore();
const runtimeStore = useRuntimeStore();
const activeGroups = ref<string[]>([]);
const busy = computed(() => store.busy.versions);
const running = computed(() => store.versionCheck.running);
const progressText = computed(() => {
  if (!running.value) return "尚未开始";
  const p = runtimeStore.progress;
  if (p.total > 0) return `处理中 ${p.processed} / ${p.total}`;
  return "扫描与聚合中…";
});
const defaultKeepText = computed(() =>
  configStore.settings.keepPolicy === "oldest"
    ? "默认每组保留最低版本"
    : "默认每组保留最高版本"
);

const selectedFiles = computed(() =>
  store.versionGroups.flatMap((group) =>
    group.files.filter((file) => file.selectedForDelete).map((file) => file.filePath)
  )
);

function baseName(path: string) {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function keepFileName(group: ModVersionGroup) {
  const keep = group.files.find((file) => !file.selectedForDelete);
  return keep ? `${keep.version || "unknown"} · ${baseName(keep.filePath)}` : "（未指定）";
}

function keepLatest(group: ModVersionGroup) {
  group.files.forEach((file) => (file.selectedForDelete = file.version !== group.latestVersion));
}

function setKeepByFile(group: ModVersionGroup, row: ModIdentityFile) {
  group.files.forEach((file) => (file.selectedForDelete = file.filePath !== row.filePath));
}

function keepLatestAll() {
  store.versionGroups.forEach(keepLatest);
}

async function preview() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  await store.previewVersions(normalized);
  activeGroups.value = [];
  ElMessage.success("检查已开始");
}

async function openFolder(filePath: string) {
  await revealInExplorer(filePath);
}

async function copyCell(row: ModIdentityFile, column: { property?: string }) {
  const key = column?.property;
  if (!key) return;
  const value = row[key as keyof ModIdentityFile];
  if (value == null) return;
  await copyText(String(value));
  ElMessage.success("已复制");
}

async function applyDelete() {
  if (!selectedFiles.value.length) {
    ElMessage.warning("请先勾选要删除的旧版本 MOD");
    return;
  }

  await ElMessageBox.confirm(
    `确认删除 ${selectedFiles.value.length} 个不同版本 MOD？删除后可从 Mod 操作记录撤回。`,
    "确认删除",
    { type: "warning" }
  );

  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  const result = await store.applyVersionDelete(normalized, selectedFiles.value);
  await store.refreshRecords();
  ElMessage.success(`删除完成：成功 ${result.success}，失败 ${result.failed}`);
}

async function rollbackLast() {
  const id = store.versionApplyResult?.recordId;
  if (!id) {
    ElMessage.warning("没有可撤回记录");
    return;
  }
  if (store.versionApplyResult && !store.versionApplyResult.rollbackEnabled) {
    ElMessage.warning("此记录创建时未启用回滚，无法撤回");
    return;
  }
  const check = await store.checkRollback(id);
  if (check.missingPaths.length) {
    await ElMessageBox.confirm(
      `有 ${check.missingPaths.length} 个备份文件不存在，仅撤回存在文件，继续？`,
      "缺失提示",
      { type: "warning" }
    );
  }
  const result = await store.rollback(id, null, true);
  await store.refreshRecords();
  ElMessage.success(
    `撤回完成：成功 ${result.success}，失败 ${result.failed}，跳过缺失 ${result.skippedMissing}`
  );
}

async function stopCheck() {
  await store.stopVersionCheck();
  ElMessage.info("已请求停止");
}

watch(
  () => store.versionGroups,
  () => {
    activeGroups.value = [];
  }
);
</script>

<template>
  <Panel class="mod-version-panel" :padded="true">
    <div class="toolbar">
      <div class="actions">
        <el-button :disabled="busy" @click="preview">检查不同版本 MOD</el-button>
        <el-button type="warning" plain :disabled="!running" @click="stopCheck">停止</el-button>
        <el-button :disabled="busy" @click="keepLatestAll">全部保留最新版本</el-button>
      </div>
      <div class="actions push">
        <el-button type="danger" :disabled="busy || !selectedFiles.length" @click="applyDelete">
          删除选中（{{ selectedFiles.length }}）
        </el-button>
        <el-button
          type="warning"
          plain
          :disabled="busy || !store.versionApplyResult || !store.versionApplyResult.rollbackEnabled"
          :title="store.versionApplyResult && !store.versionApplyResult.rollbackEnabled ? '此记录创建时未启用回滚' : ''"
          @click="rollbackLast"
        >
          撤回本次
        </el-button>
      </div>
    </div>

    <div class="tip">
      <span class="dot" />
      <span>{{ progressText }}；{{ defaultKeepText }}；结果会在检查完成后统一展示；删除会移动到可回滚备份路径。</span>
    </div>

    <div class="group-container ff-scroll">
      <el-collapse v-model="activeGroups" accordion>
        <el-collapse-item
          v-for="group in store.versionGroups"
          :key="group.groupId"
          :name="group.groupId"
        >
          <template #title>
            <div class="group-header">
              <span class="group-id">{{ group.guid || "unknown" }}</span>
              <span class="group-info">
                {{ group.author || "unknown" }} · 最新 {{ group.latestVersion || "unknown" }} ·
                {{ group.files.length }} 个文件 · 保留：{{ keepFileName(group) }}
              </span>
            </div>
          </template>

          <div class="group-content">
            <div class="group-actions">
              <el-button size="small" :disabled="busy" @click.stop="keepLatest(group)">保留最新版本</el-button>
            </div>

            <el-table :data="group.files" stripe border size="small" class="group-table" @cell-dblclick="copyCell">
              <el-table-column type="index" label="#" width="44" />
              <el-table-column prop="selectedForDelete" label="删除" width="60" align="center">
                <template #default="{ row }">
                  <el-checkbox v-model="row.selectedForDelete" :disabled="busy" @click.stop />
                </template>
              </el-table-column>
              <el-table-column prop="version" label="版本" width="140" resizable />
              <el-table-column prop="filePath" label="文件路径" min-width="340" resizable>
                <template #default="{ row }">
                  <PreviewPanel :path="row.filePath">
                    <button
                      type="button"
                      class="path-link"
                      :disabled="busy"
                      :title="stripWindowsExtendedPrefix(row.filePath)"
                      @click.stop="openFolder(row.filePath)"
                    >
                      {{ stripWindowsExtendedPrefix(row.filePath) }}
                    </button>
                  </PreviewPanel>
                </template>
              </el-table-column>
              <el-table-column prop="size" label="大小" width="100" resizable>
                <template #default="{ row }">{{ formatBytes(row.size) }}</template>
              </el-table-column>
              <el-table-column prop="mtime" label="修改时间" width="160" resizable>
                <template #default="{ row }">{{ formatTimestamp(row.mtime) }}</template>
              </el-table-column>
              <el-table-column label="保留" width="100" align="center">
                <template #default="{ row }">
                  <el-button size="small" text type="primary" :disabled="busy" @click.stop="setKeepByFile(group, row)">
                    保留此文件
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </el-collapse-item>
      </el-collapse>
      <el-empty v-if="!store.versionGroups.length" description="暂无不同版本 MOD" />
    </div>
  </Panel>
</template>

<style scoped>
.mod-version-panel {
  height: 100%;
  min-height: 0;
}
.toolbar,
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2);
}
.toolbar {
  flex-shrink: 0;
}
.push {
  margin-left: auto;
}
.tip {
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
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ff-accent);
}
.group-container {
  flex: 1;
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-sm);
}
.group-header {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  min-width: 0;
  padding: 0 var(--ff-space-3);
}
.group-id {
  font-family: ui-monospace, Monaco, Consolas, monospace;
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-info {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.group-content {
  padding: var(--ff-space-3);
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-2);
}
.group-actions {
  display: flex;
  gap: 6px;
}
.group-table {
  width: 100%;
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
