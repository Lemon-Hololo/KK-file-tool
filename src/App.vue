<script setup lang="ts">
/**
 * App 根：侧栏 + 主内容。
 *
 * # 布局责任
 * - `.app-shell`：grid 横向三列（侧栏 | 拖拽条 | 主内容），height: 100vh。
 * - `.app-main`：grid 纵向两行（顶部条 | 视口），min-height: 0 让 grid track 不被撑破。
 * - `.app-viewport`：视口容器，`overflow: hidden`，**外层不做纵向滚动**；每个
 *   路由页面自己决定内部滚动责任（TaskPage / RecordManagePage 走 flex 链
 *   一路透传到 VirtualTable；SettingsPage 表单长所以自己 overflow-y: auto）。
 *
 * 侧栏宽度持久化在 localStorage；低于阈值自动吸附到图标模式 56px。
 */

import { computed, ref } from "vue";
import { useStorage, useEventListener } from "@vueuse/core";
import { Document, Setting, Folder, Files } from "@element-plus/icons-vue";

const MIN_WIDTH = 60;
const MAX_WIDTH = 300;
const COLLAPSE_THRESHOLD = 110;
const DEFAULT_WIDTH = 220;

const sidebarWidth = useStorage<number>("sidebarWidth", DEFAULT_WIDTH);
const collapsed = computed(() => sidebarWidth.value <= MIN_WIDTH + 4);

const menu = [
  { path: "/", label: "任务中心", icon: Files },
  { path: "/records", label: "记录管理", icon: Folder },
  { path: "/settings", label: "配置中心", icon: Setting },
];

// ---- 拖拽 ----
const isDragging = ref(false);
let startX = 0;
let startWidth = 0;

function onDragStart(e: MouseEvent) {
  isDragging.value = true;
  startX = e.clientX;
  startWidth = sidebarWidth.value;
  document.body.style.userSelect = "none";
  document.body.style.cursor = "col-resize";
}

useEventListener(document, "mousemove", (e: MouseEvent) => {
  if (!isDragging.value) return;
  const delta = e.clientX - startX;
  sidebarWidth.value = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startWidth + delta));
});

useEventListener(document, "mouseup", () => {
  if (!isDragging.value) return;
  isDragging.value = false;
  document.body.style.userSelect = "";
  document.body.style.cursor = "";
  if (sidebarWidth.value < COLLAPSE_THRESHOLD) sidebarWidth.value = MIN_WIDTH;
});

function toggleCollapse() {
  sidebarWidth.value = collapsed.value ? DEFAULT_WIDTH : MIN_WIDTH;
}
</script>

<template>
  <div
    class="app-shell"
    :style="{ gridTemplateColumns: `${sidebarWidth}px 3px 1fr` }"
  >
    <!-- 侧栏 -->
    <aside class="app-sidebar" :class="{ 'is-collapsed': collapsed }">
      <button
        type="button"
        class="brand"
        :title="collapsed ? '展开' : 'KK File Tool'"
        @click="toggleCollapse"
      >
        <span class="brand-icon">
          <el-icon :size="20"><Document /></el-icon>
        </span>
        <span v-show="!collapsed" class="brand-text">KK File Tool</span>
      </button>

      <nav class="nav">
        <router-link
          v-for="m in menu"
          :key="m.path"
          :to="m.path"
          custom
          v-slot="{ isActive, navigate }"
        >
          <button
            type="button"
            class="nav-item"
            :class="{ 'is-active': isActive }"
            :title="collapsed ? m.label : ''"
            @click="navigate"
          >
            <span class="nav-icon">
              <el-icon :size="18"><component :is="m.icon" /></el-icon>
            </span>
            <span v-show="!collapsed" class="nav-label">{{ m.label }}</span>
          </button>
        </router-link>
      </nav>
    </aside>

    <!-- 拖拽条 -->
    <div
      class="resize-handle"
      :class="{ 'is-dragging': isDragging }"
      @mousedown.prevent="onDragStart"
    />

    <!-- 主体 -->
    <main class="app-main">
      <header class="app-topbar">
        <span class="topbar-title">文件处理平台</span>
        <span class="topbar-sub">Windows · Tauri</span>
      </header>
      <section class="app-viewport">
        <router-view />
      </section>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  display: grid;
  height: 100vh;
  overflow: hidden;
  background: var(--ff-bg-app);
}

/* ---- 侧栏 ---- */
.app-sidebar {
  background: var(--ff-bg-panel);
  border-right: 1px solid var(--ff-border-subtle);
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}

.brand {
  appearance: none;
  background: transparent;
  border: 0;
  padding: var(--ff-space-4) var(--ff-space-4);
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  min-height: 56px;
  color: var(--ff-text-primary);
  user-select: none;
}
.app-sidebar.is-collapsed .brand {
  justify-content: center;
  padding: var(--ff-space-4) 0;
}
.brand-icon {
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  background: var(--ff-accent-soft);
  color: var(--ff-accent);
  flex-shrink: 0;
}
.brand-text {
  font-weight: 700;
  font-size: var(--ff-font-xl);
  letter-spacing: 0.02em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nav {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--ff-space-2) var(--ff-space-2);
  overflow-y: auto;
  scrollbar-width: thin;
}

.nav-item {
  appearance: none;
  background: transparent;
  border: 0;
  text-align: left;
  color: var(--ff-text-secondary);
  padding: 8px 10px;
  min-height: 38px;
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: var(--ff-font-md);
  font-weight: 500;
  border-radius: var(--ff-radius-sm);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.app-sidebar.is-collapsed .nav-item {
  justify-content: center;
  padding: 8px 0;
}
.nav-item:hover:not(.is-active) {
  background: var(--ff-bg-muted);
  color: var(--ff-text-primary);
}
.nav-item.is-active {
  background: var(--ff-accent-soft);
  color: var(--ff-accent);
}
.nav-icon {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ---- 拖拽分隔条 ---- */
.resize-handle {
  cursor: col-resize;
  background: var(--ff-border-subtle);
  transition: background 0.15s;
}
.resize-handle:hover,
.resize-handle.is-dragging {
  background: var(--ff-accent);
}

/* ---- 主体 ---- */
.app-main {
  display: grid;
  grid-template-rows: 48px 1fr;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--ff-bg-app);
}

.app-topbar {
  display: flex;
  align-items: baseline;
  gap: var(--ff-space-3);
  padding: 0 var(--ff-space-5);
  border-bottom: 1px solid var(--ff-border-subtle);
  background: var(--ff-bg-panel);
}
.topbar-title {
  font-size: var(--ff-font-lg);
  font-weight: 600;
  color: var(--ff-text-primary);
}
.topbar-sub {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  font-weight: 500;
}

.app-viewport {
  padding: var(--ff-space-4);
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  /* 页面自己决定内部滚动 —— TaskPage 走 flex 链铺满，SettingsPage / RecordManagePage 自己开滚 */
}
</style>
