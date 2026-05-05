<!-- src/components/RealtimeLogPanel.vue -->
<script setup lang="ts">
/**
 * 实时日志面板。
 *
 * 手写虚拟滚动（行高 30px），单一滚动容器 `.log-scroll` 走 `overflow-y: auto`；
 * 自动跟随策略：距底 ≤5px 开启，距底 >50px 关闭。通过自有 Panel 壳布局，
 * 头部右端放 "跟随" / "清空" 操作。
 */

import { ref, watch, nextTick, computed, onBeforeUnmount } from "vue";
import { useStorage } from "@vueuse/core";
import type { TaskLogPayload } from "../types/common";
import { stripWindowsExtendedPrefix, baseName } from "../utils/path";
import { DEFAULT_LOG_MAX_LENGTH } from "../constants/app";
import { useConfigStore } from "../stores/config";
import Panel from "./common/Panel.vue";

const props = defineProps<{ logs: TaskLogPayload[] }>();

const emit = defineEmits<{
  (e: "clearLogs"): void;
}>();

const configStore = useConfigStore();

/** 当前生效的日志上限；跟随设置中心变化。 */
const logCap = computed(() => {
  const v = configStore.settings.logMaxLength;
  return typeof v === "number" && v > 0 ? v : DEFAULT_LOG_MAX_LENGTH;
});

const ROW_HEIGHT = 30;
const OVERSCAN = 15;

const autoFollow = useStorage<boolean>("logAutoFollow", true);

const clipped = computed(() => props.logs);

const scrollRef = ref<HTMLDivElement | null>(null);

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

let _userScrolling = false;

function onScroll() {
  const el = scrollRef.value;
  if (!el) return;

  scrollTop.value = el.scrollTop;

  const distFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;

  if (distFromBottom <= 5) {
    if (!autoFollow.value) {
      _userScrolling = true;
      autoFollow.value = true;
      _userScrolling = false;
    }
  } else if (distFromBottom > 50) {
    if (autoFollow.value) {
      _userScrolling = true;
      autoFollow.value = false;
      _userScrolling = false;
    }
  }
}

function scrollToBottom() {
  const el = scrollRef.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
}

let _lastLen = 0;
watch(
  () => clipped.value.length,
  (cur) => {
    if (cur === _lastLen) return;
    _lastLen = cur;
    if (autoFollow.value && cur > 0) {
      nextTick(() => nextTick(() => scrollToBottom()));
    }
  }
);

watch(autoFollow, (val) => {
  if (val && !_userScrolling && clipped.value.length > 0) {
    nextTick(() => nextTick(() => scrollToBottom()));
  }
});

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

function levelClass(level: string) {
  if (level === "ERROR") return "lvl-error";
  if (level === "WARN") return "lvl-warn";
  return "lvl-info";
}

function displayText(item: TaskLogPayload) {
  const msg = item.message || "";
  if (item.filePath) {
    return `${msg}  ${baseName(item.filePath)}`;
  }
  return msg;
}

function tooltipText(item: TaskLogPayload) {
  const msg = item.message || "";
  if (item.filePath) {
    return `${msg}  ${stripWindowsExtendedPrefix(item.filePath)}`;
  }
  return msg;
}
</script>

<template>
  <Panel class="log-panel" :padded="false" compact>
    <template #header>
      <span class="panel-title">实时日志</span>
      <span class="log-count">
        {{ clipped.length >= logCap ? `${logCap}+` : clipped.length }}
      </span>
    </template>
    <template #actions>
      <el-switch v-model="autoFollow" active-text="跟随" inline-prompt size="small" />
      <el-button size="small" text @click="emit('clearLogs')">清空</el-button>
    </template>

    <div ref="scrollRef" class="log-scroll ff-scroll" @scroll="onScroll">
      <div :style="{ height: totalHeight + 'px', position: 'relative' }">
        <div :style="{ transform: `translateY(${offsetY}px)` }">
          <div
            v-for="{ index, data: l } in visibleItems"
            :key="index"
            class="log-row"
            :style="{ height: ROW_HEIGHT + 'px' }"
          >
            <span class="log-level" :class="levelClass(l.level)">{{ l.level }}</span>
            <el-tooltip :content="tooltipText(l)" placement="top" :show-after="400" :hide-after="0">
              <span class="log-text">{{ displayText(l) }}</span>
            </el-tooltip>
          </div>
        </div>
      </div>
    </div>

  </Panel>
</template>

<style scoped>
.log-panel {
  height: 100%;
  width: 100%;
}

.panel-title {
  font-size: var(--ff-font-lg);
  font-weight: 600;
}

.log-count {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  background: var(--ff-bg-muted);
  padding: 1px 8px;
  border-radius: 999px;
}

.log-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 2px 0;
  background: var(--ff-bg-subtle);
}

.log-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  font-size: var(--ff-font-sm);
  box-sizing: border-box;
  font-family: ui-monospace, 'SF Mono', Monaco, Consolas, monospace;
}
.log-row:hover {
  background: var(--ff-bg-panel-hover);
}

.log-level {
  flex-shrink: 0;
  width: 44px;
  font-size: 10px;
  font-weight: 700;
  text-align: center;
  padding: 1px 0;
  border-radius: 3px;
  letter-spacing: 0.05em;
}
.lvl-info {
  color: var(--ff-accent);
  background: var(--ff-accent-soft);
}
.lvl-warn {
  color: var(--ff-warning);
  background: var(--ff-warning-soft);
}
.lvl-error {
  color: var(--ff-danger);
  background: var(--ff-danger-soft);
}

.log-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
  color: var(--ff-text-primary);
}
</style>
