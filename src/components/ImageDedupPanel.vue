<script setup lang="ts">
/**
 * 图片相似度去重面板。
 *
 * UI 结构对齐 [`ModGroupPanel`](./ModGroupPanel.vue)：长任务启动 → collapse 分组
 * → 每组 el-table 勾选要删除的项 → 删除选中 → 撤回本次。
 *
 * 与 ModGroupPanel 的差异：
 * - 顶部 toolbar 之后多一行"按相似度区间过滤"滑块（slider [min, 100]）；
 * - 每行最左侧显示缩略图（`convertFileSrc(absPath)` 直读本地文件，不需要后端
 *   生成预览），加 `loading="lazy"`；
 * - 表内多列：分辨率（`width × height`）、大小、修改时间；
 * - 4 个 keep 策略按钮（保留分辨率最大 / 文件最大 / 最新 / 最旧），
 *   `[组操作]` 一键全部 / 单组按钮各一份；
 * - 删除按钮文案区分回滚开关：开 = "删除并备份"，关 = "直接删除（不备份）"。
 *
 * 缩略图直接走 `convertFileSrc`，避免后端预览 IO；图像很大时浏览器内置 decode
 * 比 image 库快。
 */
import { computed, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { convertFileSrc } from "@tauri-apps/api/core";

import { useImageDedupStore } from "../stores/imageDedup";
import { useConfigStore } from "../stores/config";
import { useRuntimeStore } from "../stores/runtime";
import type { ImageDedupGroup, ImageHashFile } from "../types/imageDedup";
import { copyText } from "../utils/clipboard";
import {
  formatBytes,
  formatRollbackToast,
  formatTimestamp
} from "../utils/format";
import { stripWindowsExtendedPrefix } from "../utils/path";
import { confirmMissingPaths } from "../composables/useDangerConfirm";
import Panel from "./common/Panel.vue";
import PathPreviewLink from "./PathPreviewLink.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useImageDedupStore();
const configStore = useConfigStore();
const runtimeStore = useRuntimeStore();

const activeGroups = ref<string[]>([]);
/** UI 滑块下界：仅展示 similarity ≥ 此值的组。100 = 严格相同。 */
const similarityFilter = ref<number>(0);

const busy = computed(() => store.busy.scan || store.busy.apply);
const running = computed(() => store.scan.running);

/** 按 similarity 滑块过滤后的组。 */
const filteredGroups = computed<ImageDedupGroup[]>(() =>
  store.groups.filter((g) => g.similarity >= similarityFilter.value)
);

const selectedFiles = computed(() =>
  filteredGroups.value.flatMap((group) =>
    group.files
      .filter((file) => file.selectedForDelete)
      .map((file) => file.filePath)
  )
);

const rollbackEnabledNow = computed(
  () => configStore.settings.imageDedupRollbackEnabled
);

const deleteButtonText = computed(() => {
  const n = selectedFiles.value.length;
  return rollbackEnabledNow.value
    ? `删除并备份 (${n})`
    : `直接删除（不备份） (${n})`;
});

// ---- 进度可视化：百分比 + 已发现组数 ----
// 进度按"已完成哈希张数 / 候选总数"计算；候选收集阶段 total=0，进度条停在 0。
const currentProgress = computed(() => {
  const p = runtimeStore.progress;
  if (store.scan.taskId && p.taskId && p.taskId !== store.scan.taskId) {
    return { taskId: store.scan.taskId, stage: "", processed: 0, total: 0, percent: 0 };
  }
  return p;
});

const progressPercent = computed(() => {
  const p = currentProgress.value;
  if (p.total <= 0) return 0;
  const percent =
    Number.isFinite(p.percent) && p.percent > 0
      ? p.percent
      : (p.processed / p.total) * 100;
  return Math.min(100, Math.max(0, Math.round(percent)));
});

const progressDetailText = computed(() => {
  if (!running.value) return "";
  const p = currentProgress.value;
  const parts: string[] = ["扫描哈希"];
  if (p.total > 0) {
    parts.push(`${p.processed} / ${p.total}`);
  } else if (p.processed > 0) {
    parts.push(`已收集 ${p.processed} 张候选`);
  } else {
    parts.push("正在读取目录");
  }
  parts.push(`已发现 ${store.groups.length} 组`);
  return parts.join(" · ");
});

const policyText = computed(() => {
  switch (configStore.settings.imageDedupKeepPolicy) {
    case "largestFile":
      return "默认每组保留文件体积最大的";
    case "newest":
      return "默认每组保留修改时间最新的";
    case "oldest":
      return "默认每组保留修改时间最旧的";
    default:
      return "默认每组保留分辨率最大的";
  }
});

const filteredCountText = computed(() => {
  const total = store.groups.length;
  const visible = filteredGroups.value.length;
  if (total === visible) return `共 ${total} 组`;
  return `${visible} / ${total} 组（按相似度过滤）`;
});

function thumbSrc(file: ImageHashFile): string {
  return convertFileSrc(stripWindowsExtendedPrefix(file.filePath));
}

function fileBaseName(file: ImageHashFile): string {
  const path = stripWindowsExtendedPrefix(file.filePath);
  const idx = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return idx >= 0 ? path.slice(idx + 1) : path;
}

function dimensionText(file: ImageHashFile): string {
  return `${file.width} × ${file.height}`;
}

function keepFile(group: ImageDedupGroup, target: ImageHashFile) {
  group.files.forEach(
    (file) => (file.selectedForDelete = file.filePath !== target.filePath)
  );
}

/** 按指定策略对单组重新排序，把 keep 放到 files[0] 并标记其余为待删除。 */
function applyPolicyToGroup(group: ImageDedupGroup, policy: string) {
  const sorted = [...group.files].sort(comparePolicy(policy));
  const keep = sorted[0]?.filePath;
  // 调整原数组顺序与 selectedForDelete 标记。
  group.files.sort(comparePolicy(policy));
  group.files.forEach((file) => {
    file.selectedForDelete = file.filePath !== keep;
  });
}

function applyPolicyAll(policy: string) {
  store.groups.forEach((group) => applyPolicyToGroup(group, policy));
}

function comparePolicy(policy: string) {
  return (a: ImageHashFile, b: ImageHashFile): number => {
    switch (policy) {
      case "largestFile":
        return b.fileSize - a.fileSize;
      case "newest":
        return b.mtime - a.mtime;
      case "oldest":
        return a.mtime - b.mtime;
      default: {
        // largestResolution + tie-break by fileSize
        const areaA = a.width * a.height;
        const areaB = b.width * b.height;
        if (areaB !== areaA) return areaB - areaA;
        return b.fileSize - a.fileSize;
      }
    }
  };
}

function clearAllSelection() {
  let count = 0;
  store.groups.forEach((group) => {
    group.files.forEach((file) => {
      if (file.selectedForDelete) {
        file.selectedForDelete = false;
        count++;
      }
    });
  });
  if (count > 0) ElMessage.info(`已取消 ${count} 个勾选`);
  else ElMessage.info("当前没有已勾选的文件");
}

async function startScan() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  await store.startScan(normalized);
  activeGroups.value = [];
  ElMessage.success("扫描已开始");
}

async function stopScan() {
  await store.stopScan();
  ElMessage.info("已请求停止");
}

async function applyDelete() {
  if (!selectedFiles.value.length) {
    ElMessage.warning("请先勾选要删除的图片");
    return;
  }
  const confirmText = rollbackEnabledNow.value
    ? `确认删除 ${selectedFiles.value.length} 张相似图片？删除后可从图片去重记录撤回。`
    : `确认直接删除 ${selectedFiles.value.length} 张相似图片？\n当前已关闭"启用备份"，删除后无法撤回，请确认。`;
  await ElMessageBox.confirm(confirmText, "确认删除", { type: "warning" });

  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  const result = await store.applyDelete(normalized, selectedFiles.value);
  await store.refreshRecords();
  ElMessage.success(`删除完成：成功 ${result.success}，失败 ${result.failed}`);
}

async function rollbackLast() {
  const id = store.applyResult?.recordId;
  if (!id) {
    ElMessage.warning("没有可撤回记录");
    return;
  }
  if (store.applyResult && !store.applyResult.rollbackEnabled) {
    ElMessage.warning("此记录创建时未启用回滚，无法撤回");
    return;
  }
  const check = await store.checkRollback(id);
  await confirmMissingPaths(check.missingPaths.length, { noun: "备份文件" });
  const result = await store.rollback(id, null, true);
  await store.refreshRecords();
  ElMessage.success(formatRollbackToast(result));
}

async function copyPath(file: ImageHashFile) {
  await copyText(file.filePath);
  ElMessage.success("已复制路径");
}

function groupHeaderText(group: ImageDedupGroup) {
  const keep = group.files[0];
  const keepName = keep ? fileBaseName(keep) : "（无）";
  return `${group.files.length} 张图片 · 保留：${keepName}`;
}
</script>

<template>
  <Panel class="image-dedup-panel" :padded="true">
    <div class="toolbar">
      <div class="actions">
        <el-button :disabled="busy" @click="startScan">扫描相似图片</el-button>
        <el-button type="warning" plain :disabled="!running" @click="stopScan">
          停止
        </el-button>
        <el-dropdown trigger="click" :disabled="busy || !store.groups.length">
          <el-button :disabled="busy || !store.groups.length">
            全部按策略 keep
            <span class="dropdown-arrow">▾</span>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item @click="applyPolicyAll('largestResolution')">
                保留分辨率最大
              </el-dropdown-item>
              <el-dropdown-item @click="applyPolicyAll('largestFile')">
                保留文件最大
              </el-dropdown-item>
              <el-dropdown-item @click="applyPolicyAll('newest')">
                保留修改最新
              </el-dropdown-item>
              <el-dropdown-item @click="applyPolicyAll('oldest')">
                保留修改最旧
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button :disabled="busy || !selectedFiles.length" @click="clearAllSelection">
          取消全部勾选
        </el-button>
      </div>
      <div class="actions push">
        <el-button type="danger" :disabled="busy || !selectedFiles.length" @click="applyDelete">
          {{ deleteButtonText }}
        </el-button>
        <el-button
          type="warning"
          plain
          :disabled="busy || !store.applyResult || !store.applyResult.rollbackEnabled"
          :title="store.applyResult && !store.applyResult.rollbackEnabled ? '此记录创建时未启用回滚' : ''"
          @click="rollbackLast"
        >
          撤回本次
        </el-button>
      </div>
    </div>

    <!-- 扫描进度条：仅运行时显示。进度按"已完成哈希张数 / 候选总数"推进，
         候选收集阶段 total=0 进度条停在 0%。 -->
    <div v-if="running" class="progress-section">
      <div class="progress-bar-wrap">
        <el-progress
          :percentage="progressPercent"
          :stroke-width="8"
          :show-text="false"
          class="progress-bar"
        />
        <span class="progress-percent">{{ progressPercent }}%</span>
      </div>
      <div class="progress-detail">
        <span class="dot" />
        <span>{{ progressDetailText }}</span>
      </div>
    </div>

    <div class="tip">
      <span class="dot" />
      <span>
        {{ policyText }}；{{ filteredCountText }}；
        相似度阈值由配置中心控制（当前 {{ configStore.settings.imageDedupSimilarityThreshold }}%）；
        {{ rollbackEnabledNow ? '已启用备份，删除后可撤回' : '未启用备份，删除后不可撤回' }}。
      </span>
    </div>

    <div class="filter-row">
      <span class="filter-label">仅显示相似度 ≥</span>
      <el-slider
        v-model="similarityFilter"
        :min="0"
        :max="100"
        :step="1"
        show-input
        :show-input-controls="false"
        class="filter-slider"
      />
      <span class="filter-suffix">%</span>
    </div>

    <div class="group-container ff-scroll">
      <el-collapse v-model="activeGroups" accordion>
        <el-collapse-item
          v-for="group in filteredGroups"
          :key="group.groupId"
          :name="group.groupId"
        >
          <template #title>
            <div class="group-header">
              <span class="similarity-badge">{{ group.similarity }}%</span>
              <span class="group-info">{{ groupHeaderText(group) }}</span>
            </div>
          </template>

          <div class="group-content">
            <div class="group-actions">
              <el-button
                size="small"
                :disabled="busy"
                @click.stop="applyPolicyToGroup(group, 'largestResolution')"
              >
                保留分辨率最大
              </el-button>
              <el-button
                size="small"
                :disabled="busy"
                @click.stop="applyPolicyToGroup(group, 'largestFile')"
              >
                保留文件最大
              </el-button>
              <el-button
                size="small"
                :disabled="busy"
                @click.stop="applyPolicyToGroup(group, 'newest')"
              >
                保留最新
              </el-button>
              <el-button
                size="small"
                :disabled="busy"
                @click.stop="applyPolicyToGroup(group, 'oldest')"
              >
                保留最旧
              </el-button>
            </div>

            <el-table
              :data="group.files"
              stripe
              border
              size="small"
              class="group-table"
              row-key="filePath"
            >
              <el-table-column type="index" label="#" width="44" />
              <el-table-column label="删除" width="60" align="center">
                <template #default="{ row }">
                  <el-checkbox
                    v-model="(row as ImageHashFile).selectedForDelete"
                    :disabled="busy"
                    @click.stop
                  />
                </template>
              </el-table-column>
              <el-table-column label="缩略图" width="96" align="center">
                <template #default="{ row }">
                  <div class="thumb-wrap">
                    <img
                      :src="thumbSrc(row as ImageHashFile)"
                      class="thumb"
                      alt=""
                      loading="lazy"
                    />
                  </div>
                </template>
              </el-table-column>
              <el-table-column label="文件路径" min-width="320" resizable>
                <template #default="{ row }">
                  <PathPreviewLink :path="(row as ImageHashFile).filePath" :disabled="busy" />
                </template>
              </el-table-column>
              <el-table-column label="分辨率" width="120" resizable>
                <template #default="{ row }">{{ dimensionText(row as ImageHashFile) }}</template>
              </el-table-column>
              <el-table-column label="大小" width="100" resizable>
                <template #default="{ row }">{{ formatBytes((row as ImageHashFile).fileSize) }}</template>
              </el-table-column>
              <el-table-column label="修改时间" width="160" resizable>
                <template #default="{ row }">{{ formatTimestamp((row as ImageHashFile).mtime) }}</template>
              </el-table-column>
              <el-table-column label="哈希" width="120" resizable>
                <template #default="{ row }">
                  <span class="hash-text" :title="(row as ImageHashFile).hash">
                    {{ (row as ImageHashFile).hash.slice(0, 14) }}…
                  </span>
                </template>
              </el-table-column>
              <el-table-column label="保留" width="120" align="center">
                <template #default="{ row }">
                  <el-button
                    size="small"
                    text
                    type="primary"
                    :disabled="busy"
                    @click.stop="keepFile(group, row as ImageHashFile)"
                  >
                    保留此张
                  </el-button>
                </template>
              </el-table-column>
              <el-table-column label="操作" width="80" align="center">
                <template #default="{ row }">
                  <el-button
                    size="small"
                    text
                    :disabled="busy"
                    @click.stop="copyPath(row as ImageHashFile)"
                  >
                    复制
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </div>
        </el-collapse-item>
      </el-collapse>
      <el-empty
        v-if="!filteredGroups.length"
        :description="store.groups.length ? '当前过滤区间内无匹配组' : '暂无相似图片'"
      />
    </div>
  </Panel>
</template>

<style scoped>
.image-dedup-panel {
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
.dropdown-arrow {
  font-size: 10px;
  margin-left: 4px;
  color: var(--ff-text-muted);
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
.progress-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-accent-soft);
  flex-shrink: 0;
}
.progress-bar-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
}
.progress-bar {
  flex: 1;
  min-width: 0;
}
.progress-percent {
  font-size: var(--ff-font-sm);
  font-weight: 600;
  color: var(--ff-text-primary);
  min-width: 44px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.progress-detail {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
}
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ff-accent);
  flex-shrink: 0;
}
.filter-row {
  display: flex;
  align-items: center;
  gap: var(--ff-space-2);
  flex-shrink: 0;
}
.filter-label {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  white-space: nowrap;
}
.filter-slider {
  flex: 1;
  min-width: 0;
}
.filter-suffix {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-muted);
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
.similarity-badge {
  font-family: ui-monospace, Monaco, Consolas, monospace;
  font-size: var(--ff-font-xs);
  font-weight: 600;
  color: var(--ff-accent);
  background: var(--ff-accent-soft);
  padding: 2px 8px;
  border-radius: 999px;
  flex-shrink: 0;
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
  flex-wrap: wrap;
  gap: 6px;
}
.group-table {
  width: 100%;
}
.thumb-wrap {
  width: 80px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--ff-bg-muted);
  border-radius: var(--ff-radius-xs);
  overflow: hidden;
}
.thumb {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
.hash-text {
  font-family: ui-monospace, Monaco, Consolas, monospace;
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
}
</style>
