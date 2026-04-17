<!-- src/components/RealtimeLogPanel.vue -->
<script setup lang="ts">
import { ref, watch, nextTick, computed, onBeforeUnmount } from "vue";
import { useStorage } from "@vueuse/core";
import type { TaskLogPayload } from "../types/common";
import { stripWindowsExtendedPrefix } from "../utils/path";
import { LOG_MAX_LENGTH } from "../constants/app";

const props = defineProps<{ logs: TaskLogPayload[] }>();

const emit = defineEmits<{
  (e: "clearLogs"): void;
}>();

/**
 * 统一行高 —— 固定单行，长内容用 ellipsis + tooltip。
 * 固定行高让 scrollHeight 计算 100% 准确，彻底解决自动跟随漂移问题。
 */
const ROW_HEIGHT = 30;
const OVERSCAN = 15;

const autoFollow = useStorage<boolean>("logAutoFollow", true);

const clipped = computed(() => props.logs);

// 滚动容器 ref
const scrollRef = ref<HTMLDivElement | null>(null);

// 虚拟滚动状态
const scrollTop = ref(0);
const containerHeight = ref(400);

const totalHeight = computed(() => clipped.value.length * ROW_HEIGHT);

const visibleRange = computed(() => {
  const start = Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - OVERSCAN);
  const visibleCount = Math.ceil(containerHeight.value / ROW_HEIGHT);
  const end = Math.min(clipped.value.length, start + visibleCount + OVERSCAN * 2);
  return { start, end };
});

const visibleItems = computed(() => {
  const { start, end } = visibleRange.value;
  const items: { index: number; data: TaskLogPayload }[] = [];
  for (let i = start; i < end; i++) {
    items.push({ index: i, data: clipped.value[i] });
  }
  return items;
});

const offsetY = computed(() => visibleRange.value.start * ROW_HEIGHT);

// 处理用户滚动
let _userScrolling = false;

function onScroll() {
  const el = scrollRef.value;
  if (!el) return;

  scrollTop.value = el.scrollTop;

  const distFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;

  if (distFromBottom <= 5) {
    // 到底部：自动启用跟随
    if (!autoFollow.value) {
      _userScrolling = true;
      autoFollow.value = true;
      _userScrolling = false;
    }
  } else if (distFromBottom > 50) {
    // 离开底部：关闭跟随
    if (autoFollow.value) {
      _userScrolling = true;
      autoFollow.value = false;
      _userScrolling = false;
    }
  }
}

// 精确滚动到底部
function scrollToBottom() {
  const el = scrollRef.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

// 日志数据变化时，如果自动跟随则滚到底
let _lastLen = 0;
watch(
  () => clipped.value.length,
  (cur) => {
    if (cur === _lastLen) return;
    _lastLen = cur;
    if (autoFollow.value && cur > 0) {
      // 使用双 nextTick 确保 DOM 完成更新（wrapper 高度已重算）后再滚动
      nextTick(() => nextTick(() => scrollToBottom()));
    }
  }
);

// 手动开启跟随时立即滚到底
watch(autoFollow, (val) => {
  if (val && !_userScrolling && clipped.value.length > 0) {
    nextTick(() => nextTick(() => scrollToBottom()));
  }
});

// 观察容器高度变化
let _ro: ResizeObserver | null = null;

function initResizeObserver() {
  const el = scrollRef.value;
  if (!el || _ro) return;
  _ro = new ResizeObserver((entries) => {
    for (const entry of entries) {
      containerHeight.value = entry.contentRect.height;
    }
  });
  _ro.observe(el);
}

watch(scrollRef, (el) => {
  if (el) {
    containerHeight.value = el.clientHeight;
    initResizeObserver();
  }
});

onBeforeUnmount(() => {
  _ro?.disconnect();
  _ro = null;
});

function tagType(level: string) {
  if (level === "ERROR") return "danger";
  if (level === "WARN") return "warning";
  return "info";
}

/** 从路径中提取文件名 */
function baseName(p: string): string {
  if (!p) return p;
  const stripped = stripWindowsExtendedPrefix(p);
  const parts = stripped.split(/[\\/]/);
  return parts[parts.length - 1] || stripped;
}

/** 行内显示：message + 文件名 */
function displayText(item: TaskLogPayload) {
  const msg = item.message || "";
  if (item.filePath) {
    return `${msg}  ${baseName(item.filePath)}`;
  }
  return msg;
}

/** tooltip 显示：message + 全路径 */
function tooltipText(item: TaskLogPayload) {
  const msg = item.message || "";
  if (item.filePath) {
    return `${msg}  ${stripWindowsExtendedPrefix(item.filePath)}`;
  }
  return msg;
}
</script>

<template>
  <el-card class="log-card">
    <template #header>
      <div class="log-header">
        <span>实时日志</span>
        <div class="log-header-right">
          <span class="log-count">
            {{ clipped.length >= LOG_MAX_LENGTH ? `最新 ${LOG_MAX_LENGTH} 条` : `${clipped.length} 条` }}
          </span>
          <el-button size="small" text @click="emit('clearLogs')">清空</el-button>
          <el-switch v-model="autoFollow" active-text="跟随" inline-prompt size="small" />
        </div>
      </div>
    </template>

    <div ref="scrollRef" class="log-scroll" @scroll="onScroll">
      <!-- 撑出总高度的占位 -->
      <div :style="{ height: totalHeight + 'px', position: 'relative' }">
        <!-- 可见区域偏移 -->
        <div :style="{ transform: `translateY(${offsetY}px)` }">
          <div
            v-for="{ index, data: l } in visibleItems"
            :key="index"
            class="log-row"
            :style="{ height: ROW_HEIGHT + 'px' }"
          >
            <el-tag size="small" :type="tagType(l.level)" class="log-tag">{{ l.level }}</el-tag>
            <el-tooltip :content="tooltipText(l)" placement="top" :show-after="400" :hide-after="0">
              <span class="log-text">{{ displayText(l) }}</span>
            </el-tooltip>
          </div>
        </div>
      </div>
    </div>
  </el-card>
</template>

<style scoped>
.log-card {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.log-card :deep(.el-card__body) {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  padding: 0;
}

.log-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.log-header-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.log-count {
  font-size: 11px;
  opacity: 0.5;
}

.log-scroll {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
}

.log-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 10px;
  font-size: 12px;
  box-sizing: border-box;
}

.log-row:hover {
  background: var(--el-fill-color-lighter);
}

.log-tag {
  flex-shrink: 0;
}

.log-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
}
</style>
