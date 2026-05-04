<script setup lang="ts">
/**
 * 去重分组展示器（el-collapse 手风琴）。
 *
 * 单一纵向滚动容器在 `.group-container`——el-collapse 及内部 el-table 都不
 * 限高、不再嵌 el-scrollbar，避免"外层 + 中层 + 表内 body-wrapper"三条
 * 滚动条叠加。大分组用 `renderLimits` 分段加载，初始只渲染 50 行。
 */
import { ref, watch } from "vue";
import { useStorage } from "@vueuse/core";
import type { DuplicateGroup, FileEntry } from "../types/task";
import { formatTimestamp, formatBytes } from "../utils/format";
import PathPreviewLink from "./PathPreviewLink.vue";

const props = defineProps<{
  groups: DuplicateGroup[];
}>();

const emit = defineEmits<{
  (e: "preview", row: FileEntry): void;
}>();

const showAllSelectedWarning = useStorage<boolean>("showAllSelectedWarning", true);

function isAllSelected(group: DuplicateGroup): boolean {
  return group.files.length > 0 && group.files.every((f) => f.selectedForMove);
}

const activeGroups = ref<string[]>([]);

const renderLimits = ref<Record<string, number>>({});

function initRenderLimit(groupId: string, fileCount: number) {
  if (!renderLimits.value[groupId]) {
    renderLimits.value[groupId] = fileCount > 100 ? 50 : fileCount;
  }
}

function loadMore(groupId: string) {
  renderLimits.value[groupId] = (renderLimits.value[groupId] || 50) + 50;
}

function getVisibleFiles(group: DuplicateGroup) {
  initRenderLimit(group.groupId, group.files.length);
  const limit = renderLimits.value[group.groupId] || group.files.length;
  return group.files.slice(0, limit);
}

function hasMore(group: DuplicateGroup) {
  const limit = renderLimits.value[group.groupId] || group.files.length;
  return group.files.length > limit;
}

function baseName(path: string) {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function getKeepFileName(group: DuplicateGroup) {
  const keep = group.files.find((f) => !f.selectedForMove);
  return keep ? baseName(keep.absPath) : "（未指定）";
}

function setKeepByMode(group: DuplicateGroup, mode: "newest" | "oldest") {
  if (!group.files?.length) return;
  const sorted = [...group.files].sort((a, b) => a.mtime - b.mtime);
  const keepPath = mode === "newest" ? sorted[sorted.length - 1].absPath : sorted[0].absPath;
  group.files.forEach((f) => (f.selectedForMove = f.absPath !== keepPath));
}

function setKeepByFile(group: DuplicateGroup, row: FileEntry) {
  group.files.forEach((f) => (f.selectedForMove = f.absPath !== row.absPath));
}

watch(
  () => props.groups,
  () => {
    activeGroups.value = [];
    renderLimits.value = {};
  }
);
</script>

<template>
  <div class="group-container ff-scroll">
    <el-collapse v-model="activeGroups" accordion class="group-collapse">
      <el-collapse-item
        v-for="group in groups"
        :key="group.groupId"
        :name="group.groupId"
      >
        <template #title>
          <div class="group-header">
            <span class="group-id">{{ group.groupId.slice(0, 10) }}…</span>
            <span class="group-info">
              {{ group.files.length }} 个文件 · 保留：{{ getKeepFileName(group) }}
            </span>
            <span
              v-if="showAllSelectedWarning && isAllSelected(group)"
              class="group-warn"
            >
              全部勾选
            </span>
          </div>
        </template>

        <div class="group-content">
          <div
            v-if="showAllSelectedWarning && isAllSelected(group)"
            class="warn-banner"
          >
            <span>该分组所有文件均被勾选为待移动，没有保留任何文件。</span>
            <el-button text size="small" @click="showAllSelectedWarning = false">关闭此提示</el-button>
          </div>

          <div class="group-actions">
            <el-button size="small" @click.stop="setKeepByMode(group, 'newest')">保留最新</el-button>
            <el-button size="small" @click.stop="setKeepByMode(group, 'oldest')">保留最旧</el-button>
          </div>

          <el-table
            :data="getVisibleFiles(group)"
            stripe
            border
            size="small"
            class="group-table"
            @row-click="(row: FileEntry) => emit('preview', row)"
          >
            <el-table-column type="index" label="#" width="44" />
            <el-table-column prop="selectedForMove" label="移动" width="60" align="center">
              <template #default="{ row }">
                <el-checkbox v-model="row.selectedForMove" @click.stop />
              </template>
            </el-table-column>
            <el-table-column prop="absPath" label="文件路径" min-width="300" resizable>
              <template #default="{ row }">
                <PathPreviewLink :path="row.absPath" />
              </template>
            </el-table-column>
            <el-table-column prop="size" label="大小" width="100" resizable>
              <template #default="{ row }">{{ formatBytes(row.size) }}</template>
            </el-table-column>
            <el-table-column prop="mtime" label="修改时间" width="160" resizable>
              <template #default="{ row }">{{ formatTimestamp(row.mtime) }}</template>
            </el-table-column>
            <el-table-column prop="ctime" label="创建时间" width="160" resizable>
              <template #default="{ row }">{{ formatTimestamp(row.ctime) }}</template>
            </el-table-column>
            <el-table-column label="保留" width="100" align="center">
              <template #default="{ row }">
                <el-button size="small" text type="primary" @click.stop="setKeepByFile(group, row)">
                  保留此文件
                </el-button>
              </template>
            </el-table-column>
            <el-table-column prop="fromHistory" label="来源" width="70" align="center">
              <template #default="{ row }">
                <el-tag size="small" v-if="row.fromHistory">历史</el-tag>
                <el-tag size="small" type="success" v-else>当前</el-tag>
              </template>
            </el-table-column>
          </el-table>

          <div v-if="hasMore(group)" class="load-more">
            <el-button size="small" @click="loadMore(group.groupId)">
              加载更多（剩余 {{ group.files.length - (renderLimits[group.groupId] || 0) }}）
            </el-button>
          </div>
        </div>
      </el-collapse-item>
    </el-collapse>
  </div>
</template>

<style scoped>
.group-container {
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--ff-bg-panel);
}

.group-collapse {
  border: 0;
}
.group-collapse :deep(.el-collapse-item) {
  border-bottom: 1px solid var(--ff-border-subtle);
}
.group-collapse :deep(.el-collapse-item__header) {
  padding: 0 var(--ff-space-3);
  background: var(--ff-bg-panel);
}
.group-collapse :deep(.el-collapse-item__wrap) {
  background: var(--ff-bg-subtle);
}

.group-header {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 4px 0;
  min-width: 0;
}

.group-id {
  font-family: ui-monospace, Monaco, Consolas, monospace;
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  flex-shrink: 0;
}

.group-info {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-warn {
  font-size: 11px;
  color: var(--ff-danger);
  background: var(--ff-danger-soft);
  padding: 2px 8px;
  border-radius: var(--ff-radius-sm);
  font-weight: 600;
  flex-shrink: 0;
  margin-left: auto;
}

.group-content {
  padding: var(--ff-space-3);
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-2);
}

.warn-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 10px;
  background: var(--ff-warning-soft);
  color: var(--ff-warning);
  border-radius: var(--ff-radius-sm);
  font-size: var(--ff-font-sm);
}

.group-actions {
  display: flex;
  gap: 6px;
}

.group-table {
  width: 100%;
}
.group-table :deep(.el-table__row) {
  cursor: pointer;
}

.load-more {
  display: flex;
  justify-content: center;
  margin-top: 4px;
}
</style>
