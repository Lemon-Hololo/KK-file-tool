<script setup lang="ts">
import { ref, watch } from "vue";
import { useStorage } from "@vueuse/core";
import type { DuplicateGroup, FileEntry } from "../types/task";
import { formatTimestamp, formatBytes } from "../utils/format";
import { stripWindowsExtendedPrefix } from "../utils/path";
import PreviewPanel from "./PreviewPanel.vue";

const props = defineProps<{
  groups: DuplicateGroup[];
}>();

const emit = defineEmits<{
  (e: "preview", row: FileEntry): void;
}>();

// 是否显示"全部勾选"警告（用户可暂时关闭，使用 useStorage 持久化）
const showAllSelectedWarning = useStorage<boolean>("showAllSelectedWarning", true);

// 检测某个分组是否所有文件都被勾选
function isAllSelected(group: DuplicateGroup): boolean {
  return group.files.length > 0 && group.files.every((f) => f.selectedForMove);
}

// 当前展开的分组
const activeGroups = ref<string[]>([]);

// 每个分组的渲染限制（分段加载）
const renderLimits = ref<Record<string, number>>({});

// 初始化渲染限制
function initRenderLimit(groupId: string, fileCount: number) {
  if (!renderLimits.value[groupId]) {
    renderLimits.value[groupId] = fileCount > 100 ? 50 : fileCount;
  }
}

// 加载更多文件
function loadMore(groupId: string) {
  renderLimits.value[groupId] = (renderLimits.value[groupId] || 50) + 50;
}

// 获取分组的可见文件
function getVisibleFiles(group: DuplicateGroup) {
  initRenderLimit(group.groupId, group.files.length);
  const limit = renderLimits.value[group.groupId] || group.files.length;
  return group.files.slice(0, limit);
}

// 是否有更多文件
function hasMore(group: DuplicateGroup) {
  const limit = renderLimits.value[group.groupId] || group.files.length;
  return group.files.length > limit;
}

// 工具函数
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
  const keepPath =
    mode === "newest" ? sorted[sorted.length - 1].absPath : sorted[0].absPath;
  group.files.forEach((f) => (f.selectedForMove = f.absPath !== keepPath));
}

function setKeepByFile(group: DuplicateGroup, row: FileEntry) {
  group.files.forEach((f) => (f.selectedForMove = f.absPath !== row.absPath));
}

// 监听分组变化，重置展开状态
watch(
  () => props.groups,
  () => {
    activeGroups.value = [];
    renderLimits.value = {};
  }
);

</script>

<template>
  <div class="group-container">
    <el-scrollbar>
      <el-collapse v-model="activeGroups" accordion>
        <el-collapse-item v-for="group in groups" :key="group.groupId" :name="group.groupId">
          <template #title>
            <div class="group-header">
              <span class="group-id">{{ group.groupId }}</span>
              <span class="group-info">
                {{ group.files.length }} 个文件 | 保留: {{ getKeepFileName(group) }}
              </span>
              <el-tag v-if="showAllSelectedWarning && isAllSelected(group)" type="danger" size="small" style="margin-left:8px;">
                全部勾选
              </el-tag>
            </div>
          </template>

          <div class="group-content">
            <el-alert
              v-if="showAllSelectedWarning && isAllSelected(group)"
              title="该分组所有文件均被勾选为待移动，没有保留任何文件！"
              type="warning"
              show-icon
              closable
              @close="showAllSelectedWarning = false"
              style="margin-bottom: 12px;"
            />
            <div class="group-actions">
              <el-button size="small" @click.stop="setKeepByMode(group, 'newest')">
                保留最新
              </el-button>
              <el-button size="small" @click.stop="setKeepByMode(group, 'oldest')">
                保留最旧
              </el-button>
            </div>

            <el-table :data="getVisibleFiles(group)" stripe border size="small" style="width: 100%"
              @row-click="(row: FileEntry) => emit('preview', row)">
              <el-table-column type="index" label="#" width="50" />

              <el-table-column prop="selectedForMove" label="移动" width="70" align="center">
                <template #default="{ row }">
                  <el-checkbox v-model="row.selectedForMove" @click.stop />
                </template>
              </el-table-column>

              <el-table-column prop="absPath" label="文件路径" min-width="300" resizable>
                <template #default="{ row }">
                  <PreviewPanel :path="row.absPath">
                    <span
                      style="cursor: pointer; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: block;">
                      {{ stripWindowsExtendedPrefix(row.absPath) }}
                    </span>
                  </PreviewPanel>
                </template>
              </el-table-column>

              <el-table-column prop="size" label="大小" width="110" resizable>
                <template #default="{ row }">
                  {{ formatBytes(row.size) }}
                </template>
              </el-table-column>

              <el-table-column prop="mtime" label="修改时间" width="170" resizable>
                <template #default="{ row }">
                  {{ formatTimestamp(row.mtime) }}
                </template>
              </el-table-column>

              <el-table-column prop="ctime" label="创建时间" width="170" resizable>
                <template #default="{ row }">
                  {{ formatTimestamp(row.ctime) }}
                </template>
              </el-table-column>

              <el-table-column label="保留" width="120" align="center">
                <template #default="{ row }">
                  <el-button size="small" text type="primary" @click.stop="setKeepByFile(group, row)">
                    保留此文件
                  </el-button>
                </template>
              </el-table-column>

              <el-table-column prop="fromHistory" label="来源" width="90" align="center">
                <template #default="{ row }">
                  <el-tag size="small" v-if="row.fromHistory">历史</el-tag>
                  <el-tag size="small" type="success" v-else>当前</el-tag>
                </template>
              </el-table-column>
            </el-table>

            <div v-if="hasMore(group)" class="load-more">
              <el-button size="small" @click="loadMore(group.groupId)">
                加载更多 ({{ group.files.length - (renderLimits[group.groupId] || 0) }} 剩余)
              </el-button>
            </div>
          </div>
        </el-collapse-item>
      </el-collapse>
    </el-scrollbar>
  </div>
</template>

<style scoped>
.group-container {
  height: 100%;
  min-height: 0;
}

.group-header {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 4px 0;
}

.group-id {
  font-weight: 600;
  font-size: 14px;
}

.group-info {
  font-size: 12px;
  color: var(--muted-text);
}

.group-content {
  padding: 8px 0;
}

.group-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.load-more {
  display: flex;
  justify-content: center;
  margin-top: 12px;
}

:deep(.el-table) {
  cursor: pointer;
}

:deep(.el-table__body-wrapper) {
  max-height: 400px;
  overflow-y: auto;
}
</style>
