<!-- src/App.vue -->
<script setup lang="ts">
import { computed, ref } from "vue";
import { useStorage, useEventListener } from "@vueuse/core";
import {
  Document,
  Setting,
  Folder,
  Files,
} from "@element-plus/icons-vue";

// 宽度常量
const MIN_WIDTH = 56;   // 折叠图标模式宽度
const MAX_WIDTH = 320;  // 最大可拖宽度
const COLLAPSE_THRESHOLD = 100; // 低于此宽度吸附为图标模式
const DEFAULT_WIDTH = 230;

// 持久化侧边栏宽度
const sidebarWidth = useStorage<number>("sidebarWidth", DEFAULT_WIDTH);

// 是否处于折叠（图标）模式
const collapsed = computed(() => sidebarWidth.value <= MIN_WIDTH + 4);

// 菜单配置，集中管理图标和路由
const menuItems = [
  { index: "/", label: "任务中心", icon: Files },
  { index: "/settings", label: "配置中心", icon: Setting },
  { index: "/records", label: "记录管理", icon: Folder },
];

// ----- 拖拽逻辑 -----
const isDragging = ref(false);
let startX = 0;
let startWidth = 0;

function onDragStart(e: MouseEvent) {
  isDragging.value = true;
  startX = e.clientX;
  startWidth = sidebarWidth.value;
  // 拖拽时禁止文字选中
  document.body.style.userSelect = "none";
  document.body.style.cursor = "col-resize";
}

useEventListener(document, "mousemove", (e: MouseEvent) => {
  if (!isDragging.value) return;
  const delta = e.clientX - startX;
  const next = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startWidth + delta));
  sidebarWidth.value = next;
});

useEventListener(document, "mouseup", () => {
  if (!isDragging.value) return;
  isDragging.value = false;
  document.body.style.userSelect = "";
  document.body.style.cursor = "";

  // 低于折叠阈值时吸附到图标模式宽度
  if (sidebarWidth.value < COLLAPSE_THRESHOLD) {
    sidebarWidth.value = MIN_WIDTH;
  }
});

// 点击折叠图标区域展开（双击侧边栏 brand 区域切换）
function toggleCollapse() {
  sidebarWidth.value = collapsed.value ? DEFAULT_WIDTH : MIN_WIDTH;
}
</script>

<template>
  <div class="app-root" :style="{ gridTemplateColumns: sidebarWidth + 'px 4px 1fr' }">
    <!-- 侧边栏 -->
    <aside class="app-sidebar" :class="{ 'is-collapsed': collapsed }">
      <!-- Brand -->
      <div class="brand" @click="toggleCollapse" :title="collapsed ? '展开侧边栏' : 'FileFlow Desktop'">
        <el-icon :size="22">
          <Document />
        </el-icon>
        <span v-show="!collapsed" class="brand-text">FileFlow Desktop</span>
      </div>

      <!-- 导航菜单 -->
      <el-menu :default-active="$route.path" :collapse="collapsed" :collapse-transition="false" router>
        <el-menu-item v-for="item in menuItems" :key="item.index" :index="item.index"
          :title="collapsed ? item.label : ''">
          <el-icon>
            <component :is="item.icon" />
          </el-icon>
          <template #title>{{ item.label }}</template>
        </el-menu-item>
      </el-menu>
    </aside>

    <!-- 拖拽分隔条 -->
    <div class="resize-handle" :class="{ 'is-dragging': isDragging }" @mousedown.prevent="onDragStart" />

    <!-- 主内容区 -->
    <main class="app-main">
      <header class="app-header">
        <span>Windows 文件处理平台</span>
      </header>
      <section class="app-content">
        <router-view />
      </section>
    </main>
  </div>
</template>

<style scoped>
.app-root {
  display: grid;
  /* grid-template-columns 由 :style 动态注入 */
  height: 100vh;
  overflow: hidden;
}

/* ---- 侧边栏 ---- */
.app-sidebar {
  border-right: none;
  /* 由 resize-handle 充当分隔 */
  overflow: hidden;
  /* 折叠时不溢出 */
  display: flex;
  flex-direction: column;
  transition: none;
  /* 拖拽时不要 transition，防止卡顿 */
}

/* Element Plus el-menu 折叠时宽度固定为 64px，
   我们用 MIN_WIDTH=56 + 分隔条 4px，视觉上等于 60px，够放图标 */
.app-sidebar :deep(.el-menu) {
  border-right: none;
  width: 100% !important;
  /* 覆盖 el-menu 的 inline width */
}

/* 折叠时 el-menu--collapse 默认 64px，强制跟随父宽 */
.app-sidebar.is-collapsed :deep(.el-menu--collapse) {
  width: 100% !important;
}

/* ---- Brand ---- */
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  font-size: 17px;
  font-weight: 700;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  flex-shrink: 0;
  user-select: none;
}

.app-sidebar.is-collapsed .brand {
  justify-content: center;
  padding: 14px 0;
}

.brand-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- 拖拽分隔条 ---- */
.resize-handle {
  width: 4px;
  cursor: col-resize;
  background: var(--el-border-color);
  transition: background 0.15s;
  flex-shrink: 0;
}

.resize-handle:hover,
.resize-handle.is-dragging {
  background: var(--el-color-primary);
}

/* ---- 主内容 ---- */
.app-main {
  display: grid;
  grid-template-rows: 54px 1fr;
  overflow: hidden;
  min-width: 0;
  /* 防止内容撑破 grid */
}

.app-header {
  display: flex;
  align-items: center;
  padding: 0 16px;
  border-bottom: 1px solid var(--el-border-color);
  color: var(--muted-text);
  flex-shrink: 0;
}

.app-content {
  padding: 16px;
  overflow: auto;
}
</style>
