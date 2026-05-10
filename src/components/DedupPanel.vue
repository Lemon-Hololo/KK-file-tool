<script setup lang="ts">
/**
 * 去重功能面板。
 *
 * 顶部：统计 + 任务控制（历史记录、开始/暂停/继续/停止、进度条）
 * 主体：去重结果卡片（DuplicateGroupTable 走 `el-collapse`）
 *
 * 布局通过自有 Panel 包壳，flex column 一路到 DuplicateGroupTable 的单一滚动容器。
 */
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { ArrowDown, Files, DeleteFilled } from "@element-plus/icons-vue";
import { refDebounced, useStorage } from "@vueuse/core";

import { useTaskStore } from "../stores/task";
import { useRuntimeStore } from "../stores/runtime";
import { useConfigStore } from "../stores/config";
import { useRecordStore } from "../stores/record";

import type { DedupConfig, FileEntry } from "../types/task";
import type { MoveSummary } from "../types/moveReport";
import { DEFAULT_GROUP_PAGE_SIZE, GROUP_PAGE_SIZES } from "../constants/task";
import { formatBytes } from "../utils/format";
import { stripWindowsExtendedPrefix } from "../utils/path";
import { getMoveSummary } from "../services/task";

import Panel from "./common/Panel.vue";
import TaskControlPanel from "./TaskControlPanel.vue";
import DuplicateGroupTable from "./DuplicateGroupTable.vue";
import MoveConfirmDialog from "./MoveConfirmDialog.vue";
import MoveReportDialog from "./MoveReportDialog.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const emit = defineEmits<{
  (e: "preview", row: FileEntry): void;
}>();

const taskStore = useTaskStore();
const runtimeStore = useRuntimeStore();
const configStore = useConfigStore();
const recordStore = useRecordStore();

const keyword = ref("");
const keywordDebounced = refDebounced(keyword, 180);

const page = ref(1);
const pageSize = useStorage<number>("dedupPageSize", DEFAULT_GROUP_PAGE_SIZE);

const confirmDialogVisible = ref(false);
const moveSummary = ref<MoveSummary | null>(null);
const reportDialogVisible = ref(false);

const selectedRecordId = computed({
  get: () => recordStore.selectedRecordId,
  set: (v: string) => recordStore.select(v || "")
});

const dedupConfig = computed<DedupConfig>(() => ({
  keepPolicy: configStore.settings.keepPolicy,
  moveTargetPath: configStore.settings.moveTargetPath || "",
  autoSelectEnabled: true,
  saveRecordEnabled: configStore.settings.saveRecordEnabled,
  useLastRecordEnabled:
    configStore.settings.useLastRecordEnabled || !!recordStore.selectedRecordId,
  selectedRecordId: recordStore.selectedRecordId || null,
  includeCurrentFolderDuplicates: configStore.settings.includeCurrentFolderDuplicates,
  recordName: null
}));

const filteredGroups = computed(() => {
  const kw = keywordDebounced.value.trim().toLowerCase();
  if (!kw) return taskStore.resultGroups;
  return taskStore.resultGroups
    .map((g) => ({
      ...g,
      files: g.files.filter((f) => f.absPath.toLowerCase().includes(kw))
    }))
    .filter((g) => g.files.length > 0);
});

const pagedGroups = computed(() => {
  const start = (page.value - 1) * pageSize.value;
  return filteredGroups.value.slice(start, start + pageSize.value);
});

const selectedFiles = computed(() =>
  taskStore.resultGroups.flatMap((g) =>
    g.files.filter((f) => f.selectedForMove).map((f) => f.absPath)
  )
);

const duplicateGroupCount = computed(() => taskStore.resultGroups.length);
const duplicateFileCount = computed(() =>
  taskStore.resultGroups.reduce((a, g) => a + g.files.length, 0)
);
const reclaimSizeText = computed(() => formatBytes(taskStore.selectedMoveBytes));

const isRunning = computed(
  () => runtimeStore.status === "Running" || runtimeStore.status === "Paused"
);

function applyKeepModeAll(mode: "newest" | "oldest") {
  taskStore.resultGroups.forEach((g) => {
    if (!g.files?.length) return;
    const sorted = [...g.files].sort((a, b) => a.mtime - b.mtime);
    const keepPath =
      mode === "newest" ? sorted[sorted.length - 1].absPath : sorted[0].absPath;
    g.files.forEach((f) => (f.selectedForMove = f.absPath !== keepPath));
  });
  ElMessage.success(mode === "newest" ? "已全局应用：保留最新" : "已全局应用：保留最旧");
}

function clearAllSelection() {
  let count = 0;
  taskStore.resultGroups.forEach((g) => {
    g.files.forEach((f) => {
      if (f.selectedForMove) {
        f.selectedForMove = false;
        count++;
      }
    });
  });
  if (count > 0) ElMessage.info(`已取消 ${count} 个勾选`);
  else ElMessage.info("当前没有已勾选的文件");
}

function selectByFolder(folderPath: string) {
  const prefix = folderPath.replace(/\\/g, "/").replace(/\/$/, "") + "/";
  let count = 0;
  taskStore.resultGroups.forEach((g) => {
    g.files.forEach((f) => {
      const normalized = f.absPath.replace(/\\/g, "/");
      if (normalized.startsWith(prefix)) {
        f.selectedForMove = true;
        count++;
      }
    });
  });
  if (count > 0) ElMessage.success(`已勾选 ${count} 个文件（来自 ${folderPath}）`);
  else ElMessage.info("该文件夹下没有找到重复文件");
}

async function startDedupTask() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  await taskStore.start(normalized, dedupConfig.value);
  ElMessage.success("去重任务已开始");
}

async function openMoveConfirm() {
  if (!selectedFiles.value.length) {
    ElMessage.warning("请先勾选待移动文件");
    return;
  }
  moveSummary.value = await getMoveSummary(
    selectedFiles.value,
    configStore.settings.moveTargetPath || null
  );
  confirmDialogVisible.value = true;
}

async function confirmMove() {
  confirmDialogVisible.value = false;
  // 把任务输入路径传给后端，开启"保留源目录结构"时后端用它计算每个文件的相对子目录。
  const resp = await taskStore.moveSelected(
    selectedFiles.value,
    configStore.settings.moveTargetPath || null,
    props.paths
  );
  if (!resp) return;
  reportDialogVisible.value = true;
  ElMessage.success(
    `移动完成：成功 ${resp.report.totalSuccess}，失败 ${resp.report.totalFailed}`
  );
}
</script>

<template>
  <div class="dedup-panel">
    <!-- 统计 + 控制卡片 -->
    <div class="meta-card">
      <div class="stats">
        <div class="stat">
          <Files class="stat-icon" />
          <div class="stat-detail">
            <div class="stat-value">{{ duplicateGroupCount }}</div>
            <div class="stat-label">重复组</div>
          </div>
        </div>
        <div class="stat-divider" />
        <div class="stat">
          <DeleteFilled class="stat-icon" />
          <div class="stat-detail">
            <div class="stat-value">{{ duplicateFileCount }}</div>
            <div class="stat-label">重复文件</div>
          </div>
        </div>
        <div class="stat-divider" />
        <div class="stat is-highlight">
          <div class="stat-detail">
            <div class="stat-value">{{ reclaimSizeText }}</div>
            <div class="stat-label">可释放</div>
          </div>
        </div>
      </div>

      <div class="control">
        <label class="inline-field">
          <span class="field-label">历史记录</span>
          <el-select
            v-model="selectedRecordId"
            clearable
            placeholder="可选"
            size="small"
            class="record-select"
          >
            <el-option
              v-for="r in recordStore.list"
              :key="r.recordId"
              :label="`${r.recordName} (${r.entryCount})`"
              :value="r.recordId"
            />
          </el-select>
        </label>
        <TaskControlPanel
          :status="runtimeStore.status"
          :stage="runtimeStore.progress.stage"
          @start="startDedupTask"
          @pause="runtimeStore.pause"
          @resume="runtimeStore.resume"
          @stop="runtimeStore.stop"
        />
      </div>

      <div v-if="isRunning || runtimeStore.progress.percent > 0" class="progress-row">
        <span class="progress-stage">{{ runtimeStore.progress.stage || "等待中" }}</span>
        <el-progress
          :percentage="Math.round(runtimeStore.progress.percent || 0)"
          :stroke-width="10"
          :text-inside="false"
          class="progress-bar"
        />
      </div>
    </div>

    <!-- 结果区域 -->
    <Panel class="result-card" :padded="false" compact>
      <template #header>
        <span class="panel-title">去重结果</span>
        <span v-if="duplicateGroupCount > 0" class="result-count">
          {{ filteredGroups.length }} 组
        </span>
      </template>
      <template #actions>
        <el-button size="small" @click="applyKeepModeAll('newest')">保留最新</el-button>
        <el-button size="small" @click="applyKeepModeAll('oldest')">保留最旧</el-button>
        <el-button
          size="small"
          :disabled="taskStore.selectedMoveCount === 0"
          @click="clearAllSelection"
        >
          取消全部勾选
        </el-button>
        <el-dropdown v-if="paths.length > 1" trigger="click" @command="selectByFolder">
          <el-button size="small">
            按文件夹勾选<el-icon style="margin-left:4px"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item v-for="p in paths" :key="p" :command="p">
                {{ stripWindowsExtendedPrefix(p) }}
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button size="small" type="success" @click="openMoveConfirm">
          确认移动（{{ taskStore.selectedMoveCount }}）
        </el-button>
      </template>

      <div class="result-body">
        <el-input
          v-model="keyword"
          clearable
          placeholder="按路径搜索…"
          size="small"
          class="result-search"
        />

        <div class="result-groups">
          <DuplicateGroupTable
            :groups="pagedGroups"
            @preview="(row) => emit('preview', row)"
          />
        </div>

        <div class="result-footer">
          <el-pagination
            v-model:current-page="page"
            v-model:page-size="pageSize"
            background
            small
            layout="total, prev, pager, next, sizes"
            :total="filteredGroups.length"
            :page-sizes="GROUP_PAGE_SIZES"
          />
        </div>
      </div>
    </Panel>

    <MoveConfirmDialog v-model="confirmDialogVisible" :summary="moveSummary" @confirm="confirmMove" />
    <MoveReportDialog v-model="reportDialogVisible" :report="taskStore.latestMoveReport" />
  </div>
</template>

<style scoped>
.dedup-panel {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
}

/* ---- 统计 + 控制卡 ---- */
.meta-card {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
  padding: var(--ff-space-3) var(--ff-space-4);
  background: var(--ff-bg-panel);
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-md);
  box-shadow: var(--ff-shadow-sm);
}

.stats {
  display: flex;
  align-items: center;
  gap: var(--ff-space-4);
}
.stat {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.stat-icon {
  width: 20px;
  height: 20px;
  color: var(--ff-text-muted);
  flex-shrink: 0;
}
.stat-detail {
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.stat-value {
  font-size: 18px;
  font-weight: 700;
  line-height: 1.1;
  color: var(--ff-text-primary);
}
.stat.is-highlight .stat-value {
  color: var(--ff-success);
}
.stat-label {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  font-weight: 500;
}
.stat-divider {
  width: 1px;
  height: 24px;
  background: var(--ff-border);
}

.control {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--ff-space-3);
  justify-content: space-between;
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
.record-select {
  min-width: 160px;
  max-width: 320px;
  flex: 1;
}

.progress-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.progress-stage {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  min-width: 56px;
  white-space: nowrap;
}
.progress-bar {
  flex: 1;
  min-width: 0;
}

/* ---- 结果 Panel ---- */
.result-card {
  flex: 1;
  min-height: 0;
}
.panel-title {
  font-size: var(--ff-font-lg);
  font-weight: 600;
}
.result-count {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  background: var(--ff-bg-muted);
  padding: 1px 8px;
  border-radius: 999px;
}

.result-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-2);
  padding: var(--ff-space-3);
}
.result-search {
  flex-shrink: 0;
  max-width: 480px;
}
.result-groups {
  flex: 1;
  min-height: 0;
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-sm);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.result-footer {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
}
</style>
