<script setup lang="ts">
/**
 * 任务中心页。
 *
 * # 新布局（三列）
 * - 左侧栏（固定宽度 300px）：路径输入 + 选择 + 实时日志
 * - 右侧主内容区：功能 Tab（去重 / 后缀 / Mod 工具）+ 面板内容
 *
 * 为什么三列：原来路径栏占一整行但输入框很窄，右侧大量空白；
 * 日志在右列 340px，路径 + 日志合并到左栏后空间更紧凑，
 * 主内容区可占满剩余宽度，报表展示更充分。
 *
 * # Tab 切换策略
 * 用自有 TabBar + v-show 切换（不是 el-tabs）。v-show 保留 DedupPanel / SuffixPanel
 * / ModToolsPanel 的 DOM 状态（滚动位置、选择状态），切 tab 不会重挂载。
 */

import { onMounted, ref, computed } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { useStorage } from "@vueuse/core";
import { Folder, Delete, ArrowLeft } from "@element-plus/icons-vue";

import { useTaskStore } from "../stores/task";
import { useRuntimeStore } from "../stores/runtime";
import { useRecordStore } from "../stores/record";
import { usePreviewStore } from "../stores/preview";

import { uniquePaths, stripWindowsExtendedPrefix } from "../utils/path";
import { usePathNormalize } from "../composables/usePathNormalize";
import type { FileEntry } from "../types/task";

import Panel from "../components/common/Panel.vue";
import TabBar from "../components/common/TabBar.vue";
import RealtimeLogPanel from "../components/RealtimeLogPanel.vue";
import DedupPanel from "../components/DedupPanel.vue";
import SuffixPanel from "../components/SuffixPanel.vue";
import EmptyDirsPanel from "../components/EmptyDirsPanel.vue";
import ModToolsPanel from "../components/ModToolsPanel.vue";

const taskStore = useTaskStore();
const runtimeStore = useRuntimeStore();
const recordStore = useRecordStore();
const previewStore = usePreviewStore();

const paths = useStorage<string[]>("taskPaths", []);
const activeTab = useStorage<"dedup" | "suffix" | "emptyDirs" | "mod">("taskActiveTab", "dedup");
const activeSubTab = useStorage<"rename" | "organize" | "duplicates" | "versions" | "scan">("modToolsTab", "rename");
const pathInput = ref("");

const mainTabs = [
  { label: "去重", value: "dedup" },
  { label: "后缀修改", value: "suffix" },
  { label: "空文件夹清理", value: "emptyDirs" },
  { label: "Mod 工具", value: "mod" }
];

const modSubTabs = [
  { label: "重命名", value: "rename" },
  { label: "归类", value: "organize" },
  { label: "重复 MOD", value: "duplicates" },
  { label: "不同版本", value: "versions" },
  { label: "版本限制扫描", value: "scan" }
];

const displayTabs = computed(() =>
  activeTab.value === "mod" ? modSubTabs : mainTabs
);

const activeTabValue = computed({
  get: (): string => activeTab.value === "mod" ? activeSubTab.value : activeTab.value,
  set: (v: string) => {
    if (
      activeTab.value === "mod" &&
      (v === "rename" || v === "organize" || v === "duplicates" || v === "versions" || v === "scan")
    ) {
      activeSubTab.value = v as "rename" | "organize" | "duplicates" | "versions" | "scan";
    } else if (v === "dedup" || v === "suffix" || v === "emptyDirs" || v === "mod") {
      activeTab.value = v as "dedup" | "suffix" | "emptyDirs" | "mod";
    }
  }
});

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
    ...arr.filter((x): x is string => typeof x === "string")
  ]);
}

function removePath(i: number) {
  paths.value.splice(i, 1);
}

function clearPaths() {
  paths.value = [];
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
  <div class="task-page">
    <!-- 左侧栏：路径 + 日志 -->
    <aside class="left-sidebar">
      <!-- 路径卡片 -->
      <Panel class="path-card" :padded="false" compact>
        <template #header>
          <span class="panel-title">任务输入</span>
          <span class="path-count">{{ paths.length }} 条</span>
        </template>
        <template #actions>
          <el-button size="small" text :icon="Folder" @click="pickFolders">选择</el-button>
          <el-button size="small" text :icon="Delete" :disabled="!paths.length" @click="clearPaths">清空</el-button>
        </template>

        <div class="path-card-body">
          <el-input
            v-model="pathInput"
            placeholder="输入路径后回车"
            size="small"
            class="path-input"
            clearable
            @keyup.enter="addPath"
          >
            <template #append>
              <el-button size="small" @click="addPath">添加</el-button>
            </template>
          </el-input>

          <div v-if="paths.length" class="path-tags ff-scroll">
            <el-tooltip
              v-for="(p, i) in paths"
              :key="p + i"
              :content="stripWindowsExtendedPrefix(p)"
              placement="top"
              :show-after="300"
              :hide-after="0"
            >
              <el-tag
                closable
                type="info"
                effect="plain"
                size="small"
                class="path-tag"
                @close="removePath(i)"
              >
                <span class="path-tag-text">{{ stripWindowsExtendedPrefix(p) }}</span>
              </el-tag>
            </el-tooltip>
          </div>
          <div v-else class="path-empty">暂无路径，先添加或选择文件夹</div>
        </div>
      </Panel>

      <!-- 日志卡片 -->
      <div class="log-wrapper">
        <RealtimeLogPanel :logs="runtimeStore.logs" @clear-logs="runtimeStore.clearLogs()" />
      </div>
    </aside>

    <!-- 右侧主内容区 -->
    <div class="task-main">
      <div class="main-tabs">
        <el-button
          v-if="activeTab === 'mod'"
          size="small"
          text
          :icon="ArrowLeft"
          class="back-btn"
          @click="activeTab = 'dedup'"
        >
          返回
        </el-button>
        <TabBar
          :model-value="activeTabValue"
          :items="displayTabs"
          @update:model-value="(v: string) => (activeTabValue = v)"
        />
      </div>

      <div class="main-host">
        <DedupPanel
          v-show="activeTab === 'dedup'"
          :paths="paths"
          :ensure-normalized-paths="ensureNormalizedPaths"
          @preview="openPreview"
        />
        <SuffixPanel
          v-show="activeTab === 'suffix'"
          :paths="paths"
          :ensure-normalized-paths="ensureNormalizedPaths"
        />
        <EmptyDirsPanel
          v-show="activeTab === 'emptyDirs'"
          :paths="paths"
          :ensure-normalized-paths="ensureNormalizedPaths"
        />
        <ModToolsPanel
          v-show="activeTab === 'mod'"
          :paths="paths"
          :ensure-normalized-paths="ensureNormalizedPaths"
          :active-sub-tab="activeSubTab"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.task-page {
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-columns: 300px minmax(0, 1fr);
  gap: var(--ff-space-4);
}

/* ---- 左侧栏 ---- */
.left-sidebar {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
}

.path-card {
  flex-shrink: 0;
}
.panel-title {
  font-size: var(--ff-font-base);
  font-weight: 600;
  color: var(--ff-text-primary);
}
.path-count {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  background: var(--ff-bg-muted);
  padding: 1px 8px;
  border-radius: 999px;
}
.path-card-body {
  padding: var(--ff-space-2) var(--ff-space-3);
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-2);
}
.path-input {
  width: 100%;
}
.path-tags {
  display: flex;
  flex-wrap: nowrap;
  gap: 4px;
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 2px;
  max-height: 120px;
  flex-direction: column;
  align-content: flex-start;
}
.path-tag {
  flex-shrink: 0;
  max-width: 260px;
  font-size: var(--ff-font-xs);
}
.path-tag-text {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
}
.path-empty {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  padding: 4px 2px;
}

.log-wrapper {
  flex: 1;
  min-height: 0;
  display: flex;
}

/* ---- 右侧主内容区 ---- */
.task-main {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
}
.main-tabs {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--ff-space-2);
}
.back-btn {
  color: var(--ff-text-muted);
}

/* main-host 是 relative 容器，子面板都 absolute 铺满：切 tab 时零 reflow。 */
.main-host {
  flex: 1;
  min-height: 0;
  min-width: 0;
  position: relative;
}
.main-host > * {
  position: absolute;
  inset: 0;
}

/* 响应式：窄屏时切回上下布局 */
@media (max-width: 900px) {
  .task-page {
    grid-template-columns: 1fr;
    grid-template-rows: auto minmax(0, 1fr) 200px;
  }
  .left-sidebar {
    flex-direction: row;
    gap: var(--ff-space-3);
  }
  .path-card {
    flex: 1;
  }
  .log-wrapper {
    flex: 1;
  }
}
</style>
