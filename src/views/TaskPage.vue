<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { useStorage } from "@vueuse/core";

import { useTaskStore } from "../stores/task";
import { useRuntimeStore } from "../stores/runtime";
import { useRecordStore } from "../stores/record";
import { usePreviewStore } from "../stores/preview";

import { uniquePaths, stripWindowsExtendedPrefix } from "../utils/path";
import { usePathNormalize } from "../composables/usePathNormalize";
import type { FileEntry } from "../types/task";

import RealtimeLogPanel from "../components/RealtimeLogPanel.vue";
import DedupPanel from "../components/DedupPanel.vue";
import SuffixPanel from "../components/SuffixPanel.vue";
import ModToolsPanel from "../components/ModToolsPanel.vue";
// PreviewPanel 已由 DedupPanel 内部路径列直接使用，TaskPage 无需引入

const taskStore = useTaskStore();
const runtimeStore = useRuntimeStore();
const recordStore = useRecordStore();
const previewStore = usePreviewStore();

const paths = useStorage<string[]>("taskPaths", []);
const activeTab = useStorage<"dedup" | "suffix" | "mod">("taskActiveTab", "dedup");
const pathInput = ref("");

function addPath() {
  const p = pathInput.value.trim();
  if (!p) return;
  paths.value = uniquePaths([...paths.value, p]);
  pathInput.value = "";
}

async function pickFolders() {
  const selected = await open({ directory: true, multiple: true, title: "选择文件夹" });
  if (!selected) return;
  const arr = Array.isArray(selected) ? selected : [selected];
  paths.value = uniquePaths([
    ...paths.value,
    ...arr.filter((x): x is string => typeof x === "string"),
  ]);
}

function removePath(i: number) {
  paths.value.splice(i, 1);
}

async function ensureNormalizedPaths() {
  const result = await usePathNormalize(paths.value);
  if (!result.normalizedPaths.length) {
    ElMessage.warning("没有可用路径");
    return null;
  }
  paths.value = result.normalizedPaths;
  return paths.value;
}

// 保留 @preview 点击兼容（悬浮已由 DedupPanel 内部处理）
function openPreview(row: FileEntry) {
  previewStore.open(row.absPath);
}

onMounted(async () => {
  await runtimeStore.initEvents();
  await taskStore.initEvents();
  await recordStore.refresh();
});
</script>

<template>
  <div class="task-page-layout">

    <!-- 左侧：路径输入 + 日志 -->
    <section class="left-col">
      <el-card shadow="hover" class="path-card">
        <template #header>任务输入</template>

        <el-input v-model="pathInput" placeholder="输入路径后回车" @keyup.enter="addPath">
          <template #append>
            <el-button @click="addPath">添加</el-button>
          </template>
        </el-input>

        <el-space style="margin-top:8px;">
          <el-button @click="pickFolders">选择文件夹</el-button>
          <el-button type="warning" plain @click="paths = []">清空</el-button>
        </el-space>

        <el-scrollbar height="130px" style="margin-top:10px;">
          <el-tooltip v-for="(p, i) in paths" :key="p + i" :content="stripWindowsExtendedPrefix(p)" placement="top" :show-after="300" :hide-after="0">
            <el-tag closable @close="removePath(i)"
              style="margin:4px 6px 0 0;max-width:95%;">
              <span class="path-tag-text">{{ stripWindowsExtendedPrefix(p) }}</span>
            </el-tag>
          </el-tooltip>
        </el-scrollbar>
      </el-card>

      <!-- 日志区：占据左栏所有剩余高度 -->
      <div class="log-wrap">
        <RealtimeLogPanel :logs="runtimeStore.logs" @clear-logs="runtimeStore.clearLogs()" />
      </div>
    </section>

    <!-- 右侧：功能 Tab -->
    <section class="center-col">
      <el-tabs v-model="activeTab" class="feature-tabs">
        <el-tab-pane label="去重功能" name="dedup" class="feature-pane">
          <DedupPanel :paths="paths" :ensure-normalized-paths="ensureNormalizedPaths" @preview="openPreview" />
        </el-tab-pane>

        <el-tab-pane label="后缀批量修改" name="suffix" class="feature-pane">
          <SuffixPanel :paths="paths" :ensure-normalized-paths="ensureNormalizedPaths" />
        </el-tab-pane>

        <el-tab-pane label="Mod 工具" name="mod" class="feature-pane">
          <ModToolsPanel :paths="paths" :ensure-normalized-paths="ensureNormalizedPaths" />
        </el-tab-pane>
      </el-tabs>
    </section>

  </div>
</template>

<style scoped>
.task-page-layout {
  height: calc(100vh - 54px - 32px);
  min-height: 600px;
  display: grid;
  grid-template-columns: minmax(260px, 300px) 1fr;
  gap: 12px;
  overflow: hidden;
}

/* ---- 左栏 ---- */
.left-col {
  display: grid;
  grid-template-rows: auto 1fr;
  gap: 12px;
  min-height: 0;
  overflow: hidden;
}

.path-card {
  min-height: 0;
}

.log-wrap {
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* ---- 右栏 ---- */
.center-col {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.feature-tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.feature-tabs :deep(.el-tabs__content) {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.feature-pane {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.path-tag-text {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  vertical-align: bottom;
}

@media (max-width: 960px) {
  .task-page-layout {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr;
    height: auto;
    overflow: auto;
  }

  .left-col {
    grid-template-rows: auto auto;
    height: auto;
  }

  .log-wrap {
    height: 280px;
  }
}
</style>
