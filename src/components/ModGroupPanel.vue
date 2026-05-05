<script setup lang="ts">
/**
 * Mod 分组检查通用面板。
 *
 * 重复 MOD 检查（`guid + author + version`）与不同版本检查（`guid + author`）
 * 的 UI 结构几乎一致：启动长任务 → collapse 分组 → 每组 el-table 勾选删除项 →
 * 批量删除写 Mod 操作记录 → 撤回本次。差异仅在分组标题、默认保留策略文案、
 * 表内是否显示 version 列、调用 store 的哪组 action。
 *
 * 本组件用 `kind` 参数收敛这两份 600+ 行重复模板；外层
 * `ModDuplicatePanel.vue` / `ModVersionPanel.vue` 保持薄包装，避免父级 import 漂移。
 */
import { computed, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";

import { useModToolsStore } from "../stores/modTools";
import { useConfigStore } from "../stores/config";
import { useRuntimeStore } from "../stores/runtime";
import type { ModDuplicateGroup, ModIdentityFile, ModVersionGroup } from "../types/modTools";
import { copyText } from "../utils/clipboard";
import { formatBytes, formatRollbackToast, formatTimestamp } from "../utils/format";
import { baseName } from "../utils/path";
import { confirmMissingPaths } from "../composables/useDangerConfirm";
import Panel from "./common/Panel.vue";
import PathPreviewLink from "./PathPreviewLink.vue";

const props = defineProps<{
  kind: "duplicate" | "version";
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useModToolsStore();
const configStore = useConfigStore();
const runtimeStore = useRuntimeStore();
const activeGroups = ref<string[]>([]);

const isDuplicate = computed(() => props.kind === "duplicate");
const busy = computed(() => isDuplicate.value ? store.busy.duplicates : store.busy.versions);
const running = computed(() => isDuplicate.value ? store.duplicateCheck.running : store.versionCheck.running);
const groups = computed<(ModDuplicateGroup | ModVersionGroup)[]>(() =>
  isDuplicate.value ? store.duplicateGroups : store.versionGroups
);
const lastApplyResult = computed(() =>
  isDuplicate.value ? store.duplicateApplyResult : store.versionApplyResult
);

const titleText = computed(() => isDuplicate.value ? "检查重复 MOD" : "检查不同版本 MOD");
const emptyText = computed(() => isDuplicate.value ? "暂无重复 MOD" : "暂无不同版本 MOD");
const deleteWarning = computed(() => isDuplicate.value ? "请先勾选要删除的重复 MOD" : "请先勾选要删除的旧版本 MOD");
const confirmDeleteText = computed(() =>
  isDuplicate.value
    ? `确认删除 ${selectedFiles.value.length} 个重复 MOD？删除后可从 Mod 操作记录撤回。`
    : `确认删除 ${selectedFiles.value.length} 个不同版本 MOD？删除后可从 Mod 操作记录撤回。`
);
const progressText = computed(() => {
  if (!running.value) return "尚未开始";
  const p = runtimeStore.progress;
  if (p.total > 0) return `处理中 ${p.processed} / ${p.total}`;
  return "扫描与聚合中…";
});
const defaultKeepText = computed(() => {
  if (isDuplicate.value) {
    return configStore.settings.keepPolicy === "oldest"
      ? "默认每组保留修改时间最旧的文件"
      : "默认每组保留修改时间最新的文件";
  }
  return configStore.settings.keepPolicy === "oldest"
    ? "默认每组保留最低版本"
    : "默认每组保留最高版本";
});

const selectedFiles = computed(() =>
  groups.value.flatMap((group) =>
    group.files.filter((file) => file.selectedForDelete).map((file) => file.filePath)
  )
);

function keepFileName(group: ModDuplicateGroup | ModVersionGroup) {
  const keep = group.files.find((file) => !file.selectedForDelete);
  if (!keep) return "（未指定）";
  return isDuplicate.value
    ? baseName(keep.filePath)
    : `${keep.version || "unknown"} · ${baseName(keep.filePath)}`;
}

function groupTitle(group: ModDuplicateGroup | ModVersionGroup) {
  if (isDuplicate.value) {
    const g = group as ModDuplicateGroup;
    return `${g.author || "unknown"} · ${g.version || "unknown"} · ${g.files.length} 个文件 · 保留：${keepFileName(group)}`;
  }
  const g = group as ModVersionGroup;
  return `${g.author || "unknown"} · 最新 ${g.latestVersion || "unknown"} · ${g.files.length} 个文件 · 保留：${keepFileName(group)}`;
}

function setKeepByMode(group: ModDuplicateGroup, mode: "newest" | "oldest") {
  const sorted = [...group.files].sort((a, b) => a.mtime - b.mtime);
  const keep = mode === "newest" ? sorted[sorted.length - 1] : sorted[0];
  group.files.forEach((file) => (file.selectedForDelete = file.filePath !== keep?.filePath));
}

function keepLatestVersion(group: ModVersionGroup) {
  group.files.forEach((file) => (file.selectedForDelete = file.version !== group.latestVersion));
}

function setKeepByFile(group: ModDuplicateGroup | ModVersionGroup, row: ModIdentityFile) {
  group.files.forEach((file) => (file.selectedForDelete = file.filePath !== row.filePath));
}

function applyKeepModeAll(mode: "newest" | "oldest") {
  if (!isDuplicate.value) return;
  (store.duplicateGroups as ModDuplicateGroup[]).forEach((group) => setKeepByMode(group, mode));
}

function keepLatestAll() {
  if (isDuplicate.value) return;
  (store.versionGroups as ModVersionGroup[]).forEach(keepLatestVersion);
}

async function preview() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  if (isDuplicate.value) await store.previewDuplicates(normalized);
  else await store.previewVersions(normalized);
  activeGroups.value = [];
  ElMessage.success("检查已开始");
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
    ElMessage.warning(deleteWarning.value);
    return;
  }

  await ElMessageBox.confirm(confirmDeleteText.value, "确认删除", { type: "warning" });

  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  const result = isDuplicate.value
    ? await store.applyDuplicateDelete(normalized, selectedFiles.value)
    : await store.applyVersionDelete(normalized, selectedFiles.value);
  await store.refreshRecords();
  ElMessage.success(`删除完成：成功 ${result.success}，失败 ${result.failed}`);
}

async function rollbackLast() {
  const id = lastApplyResult.value?.recordId;
  if (!id) {
    ElMessage.warning("没有可撤回记录");
    return;
  }
  if (lastApplyResult.value && !lastApplyResult.value.rollbackEnabled) {
    ElMessage.warning("此记录创建时未启用回滚，无法撤回");
    return;
  }
  const check = await store.checkRollback(id);
  await confirmMissingPaths(check.missingPaths.length, { noun: "备份文件" });
  const result = await store.rollback(id, null, true);
  await store.refreshRecords();
  ElMessage.success(formatRollbackToast(result));
}

async function stopCheck() {
  if (isDuplicate.value) await store.stopDuplicateCheck();
  else await store.stopVersionCheck();
  ElMessage.info("已请求停止");
}

watch(
  () => groups.value,
  () => {
    activeGroups.value = [];
  }
);
</script>

<template>
  <Panel class="mod-group-panel" :padded="true">
    <div class="toolbar">
      <div class="actions">
        <el-button :disabled="busy" @click="preview">{{ titleText }}</el-button>
        <el-button type="warning" plain :disabled="!running" @click="stopCheck">停止</el-button>
        <template v-if="isDuplicate">
          <el-button :disabled="busy" @click="applyKeepModeAll('newest')">全部保留最新</el-button>
          <el-button :disabled="busy" @click="applyKeepModeAll('oldest')">全部保留最旧</el-button>
        </template>
        <el-button v-else :disabled="busy" @click="keepLatestAll">全部保留最新版本</el-button>
      </div>
      <div class="actions push">
        <el-button type="danger" :disabled="busy || !selectedFiles.length" @click="applyDelete">
          删除选中（{{ selectedFiles.length }}）
        </el-button>
        <el-button
          type="warning"
          plain
          :disabled="busy || !lastApplyResult || !lastApplyResult.rollbackEnabled"
          :title="lastApplyResult && !lastApplyResult.rollbackEnabled ? '此记录创建时未启用回滚' : ''"
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
          v-for="group in groups"
          :key="group.groupId"
          :name="group.groupId"
        >
          <template #title>
            <div class="group-header">
              <span class="group-id">{{ group.guid || "unknown" }}</span>
              <span class="group-info">{{ groupTitle(group) }}</span>
            </div>
          </template>

          <div class="group-content">
            <div class="group-actions">
              <template v-if="isDuplicate">
                <el-button size="small" :disabled="busy" @click.stop="setKeepByMode(group as ModDuplicateGroup, 'newest')">保留最新</el-button>
                <el-button size="small" :disabled="busy" @click.stop="setKeepByMode(group as ModDuplicateGroup, 'oldest')">保留最旧</el-button>
              </template>
              <el-button v-else size="small" :disabled="busy" @click.stop="keepLatestVersion(group as ModVersionGroup)">保留最新版本</el-button>
            </div>

            <el-table :data="group.files" stripe border size="small" class="group-table" @cell-dblclick="copyCell">
              <el-table-column type="index" label="#" width="44" />
              <el-table-column prop="selectedForDelete" label="删除" width="60" align="center">
                <template #default="{ row }">
                  <el-checkbox v-model="row.selectedForDelete" :disabled="busy" @click.stop />
                </template>
              </el-table-column>
              <el-table-column v-if="!isDuplicate" prop="version" label="版本" width="140" resizable />
              <el-table-column prop="filePath" label="文件路径" :min-width="isDuplicate ? 360 : 340" resizable>
                <template #default="{ row }">
                  <PathPreviewLink :path="row.filePath" :disabled="busy" />
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
      <el-empty v-if="!groups.length" :description="emptyText" />
    </div>
  </Panel>
</template>

<style scoped>
.mod-group-panel {
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
</style>
