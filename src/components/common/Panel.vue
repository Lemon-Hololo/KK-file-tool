<script setup lang="ts">
/**
 * Panel —— 自有"卡片"布局原语，**替代 Element Plus 的 `el-card`**。
 *
 * # 为什么不用 el-card？
 * `el-card` 的 `__header / __body` 是写死的内部类名，想把 body 变成 flex column
 * 让 VirtualTable auto-height 生效只能走 `:deep(.el-card__body) { ... }`。
 * 每次 Element Plus 更新 CSS 都可能破坏这条覆写，并且嵌套 tabs / drawers
 * 时层层 :deep 很难排查。Panel 由我们自己控制结构，直接就是 flex column，
 * 不再和组件库内部样式打架。
 *
 * # 结构
 * ```
 * .panel
 *   ├── .panel-header  (可选，通过 header slot 或 title prop)
 *   ├── .panel-body    (flex: 1; min-height: 0; flex column)
 *   └── .panel-footer  (可选)
 * ```
 *
 * # 布局契约
 * - `.panel` 自身是 `flex column`，父链需给出可计算高度（或自己是 flex: 1
 *   的子项），内容才会按行撑开。
 * - `.panel-body` 默认 `padded=true`：带 padding + gap，适合表单 + 控件排列；
 *   把 VirtualTable / 虚拟列表作为子节点时，传 `padded=false` 让它贴边，
 *   表格自己的边框就是 Panel 的视觉边界。
 * - 想"无边框仅分隔"的扁平变体，传 `flat`。
 */

withDefaults(
  defineProps<{
    /** Header 标题；传 `header` 插槽会覆盖这个。 */
    title?: string;
    /** Body 是否带默认 padding + gap。表格类内容建议关掉。 */
    padded?: boolean;
    /** 去掉 Panel 的背景 / 边框 / 阴影；仅保留 flex 结构，用于场景嵌套。 */
    flat?: boolean;
    /** 紧凑 header（高度 36px），默认 44px。 */
    compact?: boolean;
  }>(),
  { padded: true, flat: false, compact: false }
);
</script>

<template>
  <div class="panel" :class="{ 'is-flat': flat }">
    <div
      v-if="$slots.header || $slots.actions || title"
      class="panel-header"
      :class="{ 'is-compact': compact }"
    >
      <div class="panel-header-main">
        <slot name="header">
          <span class="panel-title">{{ title }}</span>
        </slot>
      </div>
      <div v-if="$slots.actions" class="panel-actions">
        <slot name="actions" />
      </div>
    </div>

    <div class="panel-body" :class="{ 'is-padded': padded }">
      <slot />
    </div>

    <div v-if="$slots.footer" class="panel-footer">
      <slot name="footer" />
    </div>
  </div>
</template>

<style scoped>
.panel {
  background: var(--ff-bg-panel);
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-md);
  box-shadow: var(--ff-shadow-sm);
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}

.panel.is-flat {
  background: transparent;
  border: none;
  box-shadow: none;
}

.panel-header {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: var(--ff-space-3);
  padding: 0 var(--ff-space-4);
  height: 44px;
  border-bottom: 1px solid var(--ff-border-subtle);
  background: var(--ff-bg-header);
}
.panel-header.is-compact {
  height: 36px;
  padding: 0 var(--ff-space-3);
}

.panel-header-main {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: var(--ff-space-2);
  overflow: hidden;
}

.panel-title {
  font-size: var(--ff-font-lg);
  font-weight: 600;
  color: var(--ff-text-primary);
  letter-spacing: 0.01em;
}

.panel-actions {
  display: flex;
  align-items: center;
  gap: var(--ff-space-2);
  flex-shrink: 0;
}

.panel-body {
  flex: 1;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.panel-body.is-padded {
  padding: var(--ff-space-3);
  gap: var(--ff-space-3);
}

.panel-footer {
  flex-shrink: 0;
  padding: var(--ff-space-2) var(--ff-space-4);
  border-top: 1px solid var(--ff-border-subtle);
  background: var(--ff-bg-header);
}
</style>
