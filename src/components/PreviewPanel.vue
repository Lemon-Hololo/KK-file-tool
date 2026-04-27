<script setup lang="ts">
import { computed, defineComponent, h, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useElementSize } from "@vueuse/core";
import {
  ElAlert,
  ElEmpty,
  ElScrollbar,
  ElSkeleton,
  ElTable,
  ElTableColumn,
} from "element-plus";
import { Document } from "@element-plus/icons-vue";
import { usePreviewStore } from "../stores/preview";
import { PREVIEW_UNSUPPORTED_TEXT } from "../constants/preview";
import { stripWindowsExtendedPrefix } from "../utils/path";

/**
 * 双模式组件：
 *  - 独立模式（不传 path）：固定卡片，从 previewStore 读取当前文件展示
 *  - Popover 模式（传入 path）：包裹 slot 内容，鼠标悬停弹出预览浮层
 */
const props = defineProps<{
  path?: string;
}>();

const store = usePreviewStore();

// ---- 独立模式：动态内容高度 ----
const cardRef = ref<HTMLElement | null>(null);
const { height: cardHeight } = useElementSize(cardRef);
const standaloneContentHeight = computed(() =>
  cardHeight.value ? Math.max(260, cardHeight.value - 80) : 420
);

// ---- 模式判断 ----
const isPopoverMode = computed(() => props.path !== undefined);

// Popover 模式用 props.path，独立模式用 store.filePath
const safePath = computed(() =>
  stripWindowsExtendedPrefix(
    isPopoverMode.value ? (props.path ?? "") : (store.filePath ?? "")
  )
);
const imgSrc = computed(() =>
  safePath.value ? convertFileSrc(safePath.value) : ""
);

// ---- Popover 模式：悬停控制 ----
const popoverVisible = ref(false);
let enterTimer: ReturnType<typeof setTimeout> | null = null;
let leaveTimer: ReturnType<typeof setTimeout> | null = null;

function onTriggerEnter() {
  if (leaveTimer) { clearTimeout(leaveTimer); leaveTimer = null; }
  enterTimer = setTimeout(() => {
    if (props.path) store.open(props.path);
    popoverVisible.value = true;
  }, 300);
}

function onTriggerLeave() {
  if (enterTimer) { clearTimeout(enterTimer); enterTimer = null; }
  leaveTimer = setTimeout(() => { popoverVisible.value = false; }, 200);
}

function onPopoverEnter() {
  if (leaveTimer) { clearTimeout(leaveTimer); leaveTimer = null; }
}

function onPopoverLeave() {
  leaveTimer = setTimeout(() => { popoverVisible.value = false; }, 200);
}

// 路径变化时刷新（仅 Popover 已展开时）
watch(
  () => props.path,
  (p) => { if (p && popoverVisible.value) store.open(p); }
);

// -----------------------------------------------------------------------
// PreviewContent：纯渲染子组件，被独立模式和 Popover 模式共用，避免模板重复
// 定义在此处而非单独文件，是因为它与 PreviewPanel 强耦合且不对外暴露
// -----------------------------------------------------------------------
const PreviewContent = defineComponent({
  name: "PreviewContent",
  props: {
    store: { type: Object, required: true },
    safePath: { type: String, required: true },
    imgSrc: { type: String, required: true },
    contentHeight: { type: Number, required: true },
  },
  setup(p) {
    return () => {
      if (p.store.loading) {
        return h(ElSkeleton, { rows: 8, animated: true });
      }

      if (!p.store.filePath) {
        return h(ElEmpty, { description: "悬停文件行以预览内容" });
      }

      const type = p.store.data?.type ?? "none";

      // 路径提示（字号小，作为内容区顶部补充说明）
      const pathHint = h("div", { class: "pc-path" }, p.safePath);

      if (type === "text") {
        return h("div", { class: "pc-wrap" }, [
          pathHint,
          p.store.data?.truncated
            ? h(ElAlert, {
              title: "仅显示前 256KB 内容",
              type: "warning",
              showIcon: true,
              closable: false,
              style: "margin-bottom:8px",
            })
            : null,
          h(ElScrollbar, { height: p.contentHeight }, () =>
            h("pre", { class: "pc-pre" }, p.store.data?.content ?? "")
          ),
        ]);
      }

      if (type === "image") {
        return h("div", { class: "pc-wrap" }, [
          pathHint,
          h("div", { class: "pc-img-wrap", style: { height: `${p.contentHeight}px` } }, [
            h("img", { src: p.imgSrc, class: "pc-img" }),
          ]),
          h("div", { class: "pc-meta" },
            `${p.store.data?.width} x ${p.store.data?.height} | ${p.store.data?.format}`
          ),
        ]);
      }

      if (type === "archive_list") {
        return h("div", { class: "pc-wrap" }, [
          pathHint,
          p.store.data?.truncated
            ? h(ElAlert, {
              title: "条目过多，已截断展示",
              type: "warning",
              showIcon: true,
              closable: false,
              style: "margin-bottom:8px",
            })
            : null,
          h(ElTable, {
            data: p.store.data?.entries ?? [],
            size: "small",
            border: true,
            height: p.contentHeight,
          }, () => [
            h(ElTableColumn, { prop: "name", label: "内部路径" }),
            h(ElTableColumn, { prop: "size", label: "大小", width: 120 }),
            h(ElTableColumn, { prop: "isDir", label: "目录", width: 80 }),
            h(ElTableColumn, { prop: "modifiedAt", label: "修改时间", width: 170 }),
          ]),
        ]);
      }

      // 不支持的类型
      return h("div", { class: "pc-wrap" }, [
        pathHint,
        h(ElEmpty, { description: PREVIEW_UNSUPPORTED_TEXT }),
      ]);
    };
  },
});
</script>

<template>
  <!-- ========== Popover 包裹模式 ========== -->
  <el-popover
    v-if="isPopoverMode"
    :visible="popoverVisible"
    placement="right-start"
    :width="440"
    trigger="manual"
    :persistent="false"
    popper-class="preview-panel-popper"
    @mouseenter="onPopoverEnter"
    @mouseleave="onPopoverLeave"
  >
    <template #default>
      <div v-if="popoverVisible" class="pop-wrap">
        <div class="pop-title">
          <el-icon class="pop-title-icon">
            <Document />
          </el-icon>
          <span class="pop-title-text" :title="safePath">{{ safePath }}</span>
        </div>
        <div class="pop-body">
          <PreviewContent :store="store" :safe-path="safePath" :img-src="imgSrc" :content-height="360" />
        </div>
      </div>
    </template>

    <template #reference>
      <span class="pop-trigger" @mouseenter="onTriggerEnter" @mouseleave="onTriggerLeave">
        <slot />
      </span>
    </template>
  </el-popover>

  <!-- ========== 独立卡片模式 ========== -->
  <el-card v-else ref="cardRef" class="standalone-card">
    <template #header>文件预览</template>
    <PreviewContent :store="store" :safe-path="safePath" :img-src="imgSrc" :content-height="standaloneContentHeight" />
  </el-card>
</template>

<style scoped>
/* ---- 独立卡片 ---- */
.standalone-card {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* ---- Popover 触发器 ---- */
.pop-trigger {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: default;
}

/* ---- Popover 内容 ---- */
.pop-wrap {
  display: flex;
  flex-direction: column;
  max-height: 460px;
  overflow: hidden;
}

.pop-title {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  font-size: 12px;
  font-weight: 600;
  flex-shrink: 0;
  border-bottom: 1px solid var(--el-border-color-light);
}

.pop-title-icon {
  flex-shrink: 0;
}

.pop-title-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.75;
}

.pop-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  padding: 8px;
}
</style>

<!-- PreviewContent render function 产生的类名无 scoped hash，用全局样式 -->
<style>
.preview-panel-popper {
  padding: 0 !important;
}

.pc-wrap {
  display: flex;
  flex-direction: column;
}

.pc-path {
  font-size: 11px;
  color: var(--muted-text);
  margin-bottom: 8px;
  word-break: break-all;
}

.pc-pre {
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
}

.pc-meta {
  margin-top: 8px;
  font-size: 12px;
  color: var(--muted-text);
}

.pc-img-wrap {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: auto;
  border: 1px solid var(--el-border-color-light);
  border-radius: 8px;
  background: rgba(127, 127, 127, 0.06);
}

.pc-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
</style>
