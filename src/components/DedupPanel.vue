<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { ArrowDown } from "@element-plus/icons-vue";
import { refDebounced, useElementSize, useStorage } from "@vueuse/core";

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

const panelRef = ref<HTMLElement | null>(null);
const { height: panelHeight } = useElementSize(panelRef);

const keyword = ref("");
const keywordDebounced = refDebounced(keyword, 180);

const page = ref(1);
const pageSize = useStorage<number>("dedupPageSize", DEFAULT_GROUP_PAGE_SIZE);

const confirmDialogVisible = ref(false);
const moveSummary = ref<MoveSummary | null>(null);
const reportDialogVisible = ref(false);

const selectedRecordId = computed({
  get: () => recordStore.selectedRecordId,
  set: (v: string) => recordStore.select(v || ""),
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
  recordName: null,
}));

const filteredGroups = computed(() => {
  const kw = keywordDebounced.value.trim().toLowerCase();
  if (!kw) return taskStore.resultGroups;
  return taskStore.resultGroups
    .map((g) => ({
      ...g,
      files: g.files.filter((f) => f.absPath.toLowerCase().includes(kw)),
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

const isRunning = computed(() => runtimeStore.status === "Running" || runtimeStore.status === "Paused");

const resultAreaHeight = computed(() => {
  const h = panelHeight.value;
  if (!h) return 340;
  return Math.max(260, h - 380);
});

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

/** 勾选指定文件夹路径下的所有文件为"待移动" */
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
  if (count > 0) {
    ElMessage.success(`已勾选 ${count} 个文件（来自 ${folderPath}）`);
  } else {
    ElMessage.info("该文件夹下没有找到重复文件");
  }
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
  const resp = await taskStore.moveSelected(
    selectedFiles.value,
    configStore.settings.moveTargetPath || null
  );
  if (!resp) return;
  reportDialogVisible.value = true;
  ElMessage.success(
    `移动完成：成功 ${resp.report.totalSuccess}，失败 ${resp.report.totalFailed}`
  );
}
</script>

<template>
  <div ref="panelRef" class="dedup-panel">
    <!-- 顶部统计 + 控制 -->
    <div class="top-section">
      <!-- 统计行 -->
      <div class="stats-row">
        <div class="stat-item">
          <span class="stat-value">{{ duplicateGroupCount }}</span>
          <span class="stat-label">重复组</span>
        </div>
        <el-divider direction="vertical" />
        <div class="stat-item">
          <span class="stat-value">{{ duplicateFileCount }}</span>
          <span class="stat-label">重复文件</span>
        </div>
        <el-divider direction="vertical" />
        <div class="stat-item">
          <span class="stat-value highlight">{{ reclaimSizeText }}</span>
          <span class="stat-label">可释放空间</span>
        </div>
      </div>

      <!-- 任务控制 + 进度 -->
      <div class="control-section">
        <div class="control-row">
          <div class="record-select">
            <span class="control-label">历史记录</span>
            <el-select v-model="selectedRecordId" clearable placeholder="可选" size="small" style="width:200px">
              <el-option v-for="r in recordStore.list" :key="r.recordId" :label="`${r.recordName} (${r.entryCount})`"
                :value="r.recordId" />
            </el-select>
          </div>
          <TaskControlPanel :status="runtimeStore.status" :stage="runtimeStore.progress.stage" @start="startDedupTask"
            @pause="runtimeStore.pause" @resume="runtimeStore.resume" @stop="runtimeStore.stop" />
        </div>

        <div v-if="isRunning || runtimeStore.progress.percent > 0" class="progress-row">
          <span class="progress-stage">{{ runtimeStore.progress.stage || "等待中" }}</span>
          <el-progress
            :percentage="Math.round(runtimeStore.progress.percent || 0)"
            :stroke-width="16"
            :text-inside="true"
            striped
            :striped-flow="isRunning"
            style="flex:1;min-width:0"
          />
        </div>
      </div>
    </div>

    <!-- 结果区域 -->
    <el-card shadow="never" class="result-card">
      <template #header>
        <div class="result-header">
          <div class="result-header-left">
            <span class="result-title">去重结果</span>
            <el-tag v-if="duplicateGroupCount > 0" size="small" type="info">
              {{ filteredGroups.length }} 组
            </el-tag>
          </div>
          <div class="result-actions">
            <el-button size="small" @click="applyKeepModeAll('newest')">保留最新</el-button>
            <el-button size="small" @click="applyKeepModeAll('oldest')">保留最旧</el-button>
            <el-dropdown v-if="paths.length > 1" @command="selectByFolder" trigger="click">
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
          </div>
        </div>
      </template>

      <el-input
        v-model="keyword"
        clearable
        placeholder="按路径搜索..."
        size="small"
        style="margin-bottom:10px"
      />

      <div class="result-scroll" :style="{ height: `${resultAreaHeight}px` }">
        <DuplicateGroupTable :groups="pagedGroups" @preview="(row) => emit('preview', row)" />
      </div>

      <div class="pagination-row">
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
    </el-card>

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
  gap: 10px;
}

/* ---- 顶部区域 ---- */
.top-section {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: visible;
}

.stats-row {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: var(--el-fill-color-lighter);
  border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter);
}

.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  min-width: 70px;
}

.stat-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--el-text-color-primary);
  line-height: 1.2;
}

.stat-value.highlight {
  color: var(--el-color-success);
}

.stat-label {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

.control-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: visible;
}

.control-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.record-select {
  display: flex;
  align-items: center;
  gap: 8px;
}

.control-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}

.progress-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 24px;
  overflow: visible;
}

.progress-stage {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
  min-width: 60px;
}

/* ---- 结果卡片 ---- */
.result-card {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.result-card :deep(.el-card__body) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.result-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.result-title {
  font-weight: 600;
}

.result-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.result-scroll {
  flex: 1;
  min-height: 260px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  overflow: hidden;
}

.pagination-row {
  display: flex;
  justify-content: flex-end;
  margin-top: 10px;
  flex-shrink: 0;
}
</style>
