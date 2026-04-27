<script setup lang="ts">
/**
 * TabBar —— 自有"段式切换"原语，**替代 Element Plus 的 `el-tabs`**。
 *
 * # 为什么不用 el-tabs？
 * `el-tabs` 把 tab header + tab content 全包进自己的结构，想要"tab 内容撑满
 * 剩余空间"必须 `:deep(.el-tabs__content)` 覆写 `flex: 1; min-height: 0;
 * overflow: hidden`，再把 tab-pane 改 `display: flex; flex-direction: column`。
 * 这条链路每次 Element Plus 升级都可能崩，且 tab 切换时 pane 间的 display
 * 切换会搅动 flex item 的"初始高度"，触发 VirtualTable 的 ResizeObserver
 * 反复 fire，造成滚动条闪烁。
 *
 * TabBar 只负责按钮条渲染；内容切换由调用方用 v-show / v-if 自行驱动。
 * 单向数据流：`modelValue` in, `update:modelValue` out。
 *
 * # 视觉
 * 段式背板 (`.tab-bar { background: muted }`) + 选中项提到 panel 底色 +
 * 轻阴影。近似 macOS / iOS 的 Segmented Control，避免下划线 tabs 在暗色主题
 * 下太重的视觉压力。
 */

interface Item {
  /** 绑定到 modelValue 的唯一值。 */
  value: string;
  /** 显示文本。 */
  label: string;
  /** 可选右上角徽标（数字或字符串）。 */
  badge?: number | string;
  /** 可选禁用态。 */
  disabled?: boolean;
}

defineProps<{
  modelValue: string;
  items: Item[];
  /** 尺寸：normal 6px/14px padding, small 4px/10px。 */
  size?: "normal" | "small";
}>();

defineEmits<{
  (e: "update:modelValue", v: string): void;
}>();
</script>

<template>
  <div class="tab-bar" :class="`tab-bar--${size ?? 'normal'}`">
    <button
      v-for="item in items"
      :key="item.value"
      type="button"
      class="tab-btn"
      :class="{ 'is-active': modelValue === item.value, 'is-disabled': item.disabled }"
      :disabled="item.disabled"
      @click="$emit('update:modelValue', item.value)"
    >
      <span class="tab-label">{{ item.label }}</span>
      <span v-if="item.badge !== undefined && item.badge !== null && item.badge !== ''" class="tab-badge">
        {{ item.badge }}
      </span>
    </button>
  </div>
</template>

<style scoped>
.tab-bar {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  background: var(--ff-bg-muted);
  border-radius: var(--ff-radius-md);
  flex-shrink: 0;
  max-width: 100%;
  overflow-x: auto;
  scrollbar-width: none;
}
.tab-bar::-webkit-scrollbar {
  display: none;
}

.tab-btn {
  appearance: none;
  background: transparent;
  border: 0;
  color: var(--ff-text-secondary);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-weight: 500;
  font-size: var(--ff-font-md);
  padding: 6px 14px;
  border-radius: calc(var(--ff-radius-md) - 3px);
  white-space: nowrap;
  transition: background 0.15s ease, color 0.15s ease, box-shadow 0.15s ease;
}

.tab-bar--small .tab-btn {
  padding: 4px 10px;
  font-size: var(--ff-font-sm);
}

.tab-btn:hover:not(.is-active):not(.is-disabled) {
  color: var(--ff-text-primary);
  background: var(--ff-bg-panel-hover);
}

.tab-btn.is-active {
  background: var(--ff-bg-panel);
  color: var(--ff-text-primary);
  box-shadow: var(--ff-shadow-sm);
}

.tab-btn.is-disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.tab-badge {
  background: var(--ff-accent-soft);
  color: var(--ff-accent);
  padding: 0 6px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  min-width: 18px;
  text-align: center;
  line-height: 16px;
}

.tab-btn.is-active .tab-badge {
  background: var(--ff-accent);
  color: #fff;
}
</style>
