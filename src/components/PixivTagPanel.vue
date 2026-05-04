<script setup lang="ts">
/**
 * Pixiv 标签整理面板。
 *
 * 流程：
 * 1. 用户在任务输入侧边栏添加图片目录；
 * 2. 在本面板顶部填输出目录、点"开始扫描"；
 * 3. 后端先同步扫出所有含 PID 的图片建出 pending 行，再启动长任务并发拉 tag；
 * 4. 行上的"标签"单元格里所有 tag 都渲染成气泡；点气泡 → 把图移动到 `<output>/<tag>/`；
 * 5. "已移动到"列显示该行最近一次被移到的 tag；当前所在 tag 的气泡会高亮 + 置灰。
 *
 * 设计选择（这版）：
 * - **不再每 tag 一列**：列数固定 5–6 个（"显示图片"开关再加一列），气泡平铺在单个 cell 里；
 * - **行高随内容自适应**：tag-list 用 `flex-wrap: wrap` 自动换行；VirtualTable 开启
 *   `autoRowHeight` 后每行高度由 cell 内容（即气泡墙 + 缩略图）实测决定，不再需要
 *   单元格内部滚动条，所有 tag 都能直接看到；
 * - **可选缩略图列**：开关控制；开启后图片列默认排在最前面，且 fileName 列上的
 *   hover 浮窗会被去掉（缩略图常驻就够了）；
 * - **列自定义开**：传 `column-config-key="pixiv:tags"`，用户可隐藏列 / 调整顺序 /
 *   切换左固定，配置随 localStorage 持久化；
 * - **fit-width 开**：列总宽不大，让 tag 列吃掉剩余空间最舒服；
 * - **翻译开关**：与配置中心同步 `pixivUseTranslation`，开启后 chip 显示 `translation.en`、
 *   点击后图片也落在 `<output>/<en 译名>/`；缺译名的 tag 自然回落原 tag。
 *   每行 chip 的展示由 panel 级 `tagsByPid` ref 维护，`watchEffect` 监听
 *   `pixivUseTranslation` / `pixivExcludedTags` / `store.rows`（含每行的
 *   `tags + translations`），任一变更立刻重建整张 Map —— `watchEffect` 是
 *   eager 的，规避了 `computed` 在 Pinia store 嵌套对象 + Map 输出场景下偶发
 *   不重算的灰区。
 */
import { computed, onMounted, ref, watch, watchEffect } from "vue";
import { useStorage } from "@vueuse/core";
import { ElMessage } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Folder } from "@element-plus/icons-vue";

import { usePixivTagStore } from "../stores/pixivTag";
import { useConfigStore } from "../stores/config";
import { revealInExplorer } from "../services/task";
import { stripWindowsExtendedPrefix } from "../utils/path";
import { DEFAULT_EXTREME_ROW_THRESHOLD, EXTREME_OVERSCAN, NORMAL_OVERSCAN } from "../constants/task";
import type { PixivImageState } from "../types/pixivTag";
import type { VirtualColumn } from "../types/virtualTable";
import Panel from "./common/Panel.vue";
import PreviewPanel from "./PreviewPanel.vue";
import VirtualTable from "./common/VirtualTable.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = usePixivTagStore();
const configStore = useConfigStore();

const moving = ref(false);

/**
 * 是否在表里显示图片缩略图列。开启后会：
 * 1) 在所有列**最前面**插一列 96px 宽的"图片"，cell 渲染 `convertFileSrc(absPath)`；
 * 2) 把"文件"列上的 PreviewPanel hover 浮窗去掉——既然缩略图常驻，再悬浮一遍是浪费。
 *
 * 跨会话持久化在 localStorage（`pixivTag:showImage`）。
 */
const showImageColumn = useStorage<boolean>("pixivTag:showImage", false);

/**
 * `pid → 该行该展示的 tag 列表（已剔除排除项 + 字典序）`。
 *
 * 用 `watchEffect + ref` 而不是 `computed`，**专门为了让"翻译开关切换"立刻见到效果**。
 *
 * 之前是 `computed(() => { ... })`，理论上 Vue 3 的依赖追踪能在 `pixivUseTranslation`
 * 变化时重算。但实际遇到一个 bug：当切换开关时 chip 文本不刷新（仍显示原 tag）。
 * 怀疑是 `for (const row of store.rows)` 嵌套读 `row.translations[t]` 时，循环里
 * 的 `if (useT) { trans[t] ... }` 在 `useT = false` 那次执行没建立对 `trans[t]`
 * 的追踪，开关切到 true 时只重算一次然后回到稳态——Vue Reactive Proxy 在
 * Pinia store + 嵌套对象 + Map 输出的组合下偶尔不重算。
 *
 * `watchEffect + ref` 的好处：watchEffect 是 eager 的，每次依赖变化都立刻执行，
 * 不依赖 computed 的"被消费时才重算"的 lazy 语义；ref 整体替换让模板使用
 * `tagsByPid.value` 的依赖追踪非常明确（只看 ref 本身，不深挖里面 Map）。
 *
 * 翻译规则：开关开 + `row.translations[原 tag]` 是非空字符串 → 用译名；
 * 否则一律用原 tag。匹配排除项用最终展示出的字符串（开关切换后行为对用户直觉一致）。
 */
const tagsByPid = ref<Map<string, { original: string; display: string }[]>>(new Map());

watchEffect(() => {
  const useT = Boolean(configStore.settings.pixivUseTranslation);
  const ex = new Set(configStore.settings.pixivExcludedTags ?? []);
  const map = new Map<string, { original: string; display: string }[]>();
  for (const row of store.rows) {
    const items: { original: string; display: string }[] = [];
    // 显式无条件读 row.translations 与 row.tags，让 watchEffect 一定追踪到这两个
    // 字段的变化。即便 useT = false 这次循环没真正使用 trans[t]，也确保了下次切到
    // true 时依赖图已经建立 —— 不再走 computed lazy 路径上"读了又不读"的灰色地带。
    const trans = row.translations || {};
    for (const t of row.tags) {
      let display = t;
      if (useT) {
        const en = trans[t];
        if (typeof en === "string" && en.length > 0) {
          display = en;
        }
      }
      if (ex.has(display)) continue;
      items.push({ original: t, display });
    }
    items.sort((a, b) => a.display.localeCompare(b.display));
    map.set(row.pid, items);
  }
  tagsByPid.value = map;
});

const isExtreme = computed(() => {
  const th = configStore.settings.extremeRowThreshold || DEFAULT_EXTREME_ROW_THRESHOLD;
  return store.rows.length > th;
});

/**
 * 列定义。**图片列在最前**（用户开启缩略图后，最直接的视觉就在最左侧）。
 * 列自定义会持久化到 localStorage（key = `vtable:col:pixiv:tags`），用户可调整。
 *
 * 排序约定：image > fileName > pid > status > tags > movedTag。"image" 出现在
 * `props.columns` 中的源索引始终为 0，配合 VirtualTable 的"按源索引插入"reconcile，
 * 用户首次开启缩略图时图片列就会出现在最左边而不是被 append 到末尾。
 */
const columns = computed<VirtualColumn[]>(() => {
  const cols: VirtualColumn[] = [];
  if (showImageColumn.value) {
    cols.push({ key: "image", label: "图片", width: 96, slotName: "image", fixed: "left" });
  }
  cols.push(
    { key: "fileName", label: "文件", minWidth: 240, ellipsis: true, slotName: "fileName", fixed: "left", resizable: true },
    { key: "pid", label: "PID", width: 110, fixed: "left", resizable: true },
    { key: "status", label: "状态", width: 76, slotName: "status", fixed: "left" },
    { key: "tags", label: "标签", minWidth: 360, slotName: "tags", resizable: true },
    { key: "movedTag", label: "已移动到", width: 160, slotName: "movedTag", resizable: true }
  );
  return cols;
});

const statusText = computed(() => {
  if (store.running) {
    return `拉取中…（${store.completedCount} / ${store.rows.length}，错误 ${store.errorCount}）`;
  }
  if (!store.taskId && store.rows.length === 0) return "尚未开始";
  return `已完成（成功 ${store.completedCount - store.errorCount}，错误 ${store.errorCount}，共 ${store.rows.length}）`;
});

const statusClass = computed(() =>
  store.running ? "is-running" : store.rows.length ? "is-done" : "is-idle"
);

async function pickOutputDir() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择输出目录"
    });
    if (typeof selected === "string" && selected) {
      store.outputDir = selected;
    }
  } catch (e) {
    ElMessage.error(`打开目录选择失败：${String(e)}`);
  }
}

async function startScan() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;
  try {
    await store.startScan(normalized);
    if (store.rows.length === 0) {
      ElMessage.warning("没有识别到带 PID 的图片");
    } else {
      ElMessage.success(`扫描已开始：${store.rows.length} 张图片`);
    }
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function stopScan() {
  try {
    await store.stop();
    ElMessage.info("已请求停止");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function retryRow(pid: string) {
  try {
    await store.retry(pid);
  } catch (e) {
    ElMessage.error(`重试失败：${String(e)}`);
  }
}

/**
 * 点击 tag 气泡：把 `<row.absPath>` 移到 `<outputDir>/<tag>/`。
 *
 * 这里的 `tag` 参数是模板里 `t.display` —— 即 `tagsByPid` 计算出的"有效字符串"，
 * 开关开时是 en 译名、关时是原 tag。后端按这个字符串建子目录，所以 `movedTag`
 * 自然记录的就是"实际落盘的目录名"。用户翻转开关后看到 highlight 不亮是预期的
 * （chip 文本变了，但文件确实还在那个原始目录里）；想再次按当前显示文本归档
 * 可以再点一次。
 */
async function clickTag(pid: string, tag: string) {
  if (!store.outputDir) {
    ElMessage.warning("请先选择输出目录");
    return;
  }
  if (moving.value) return; // 简单避免连点
  moving.value = true;
  try {
    await store.moveByTag(pid, tag);
    ElMessage.success(`已移动到 ${tag}`);
  } catch (e) {
    ElMessage.error(`移动失败：${String(e)}`);
  } finally {
    moving.value = false;
  }
}

async function openLocation(filePath: string) {
  await revealInExplorer(filePath);
}

onMounted(() => {
  // 预先注册 partial 事件监听，避免 startScan 第一时间发的 partial 漏掉。
  store.initEvents();
});

/**
 * 翻译开关在配置中心和本面板顶部都暴露，但 SettingsPage 的 `watchDebounced`
 * 只在该页面挂载时跑——用户在本面板上切换 `pixivUseTranslation` 不会自动落库。
 * 这里专门补一个 save：只盯这一项变化，触发 store 的 `saveSettings`
 * （内部已合并 / 节流，连按多次只会写最终一次）。
 */
watch(
  () => configStore.settings.pixivUseTranslation,
  () => {
    void configStore.saveSettings();
  }
);
</script>

<template>
  <Panel class="pixiv-panel" :padded="true">
    <!-- 顶部工具条 -->
    <div class="toolbar">
      <label class="inline-field output-field">
        <span class="field-label">输出目录</span>
        <el-input
          v-model="store.outputDir"
          placeholder="点击 tag 时图片会移动到 此目录/tag/"
          clearable
        >
          <template #append>
            <el-button :icon="Folder" @click="pickOutputDir">选择</el-button>
          </template>
        </el-input>
      </label>

      <label class="inline-field">
        <span class="field-label">显示图片</span>
        <el-switch v-model="showImageColumn" />
      </label>

      <label class="inline-field">
        <!--
          翻译开关：直接绑到 configStore.settings.pixivUseTranslation，
          配置中心和这里改一处两处都生效（saveSettings 由 SettingsPage 的 watchDebounced 自动落库）。
        -->
        <span class="field-label">使用英文译名</span>
        <el-switch v-model="configStore.settings.pixivUseTranslation" />
      </label>

      <div class="actions">
        <el-button type="primary" :disabled="store.running" @click="startScan">开始扫描</el-button>
        <el-button type="warning" plain :disabled="!store.running" @click="stopScan">停止</el-button>
      </div>
    </div>

    <div class="status-bar" :class="statusClass">
      <span class="status-dot" />
      <span>{{ statusText }}</span>
    </div>

    <div v-if="!store.outputDir" class="hint-warn">
      <span class="dot dot-warn" />
      <span>未选择输出目录，标签气泡不可点击。</span>
    </div>

    <div v-if="isExtreme" class="hint-warn">
      <span class="dot dot-warn" />
      <span>结果较多，已启用极限性能模式</span>
    </div>

    <VirtualTable
      :key="showImageColumn ? 'pixiv-with-image' : 'pixiv-no-image'"
      class="pixiv-table"
      :rows="store.rows"
      :columns="columns"
      :item-height="36"
      auto-row-height
      :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
      row-key="pid"
      column-config-key="pixiv:tags"
      fit-width
      empty-text="尚无数据，点'开始扫描'识别 Pixiv 图片"
    >
      <template #fileName="{ row }">
        <!--
          showImageColumn 关：保留原有 PreviewPanel hover 浮窗（用户没法直接看到图，悬浮再展示是合理的）。
          开：缩略图常驻，悬浮浮窗就是冗余 + 容易误触，去掉它。
        -->
        <PreviewPanel v-if="!showImageColumn" :path="(row as PixivImageState).absPath">
          <button
            type="button"
            class="path-link"
            :title="stripWindowsExtendedPrefix((row as PixivImageState).absPath)"
            @click.stop="openLocation((row as PixivImageState).absPath)"
          >
            {{ (row as PixivImageState).fileName }}
          </button>
        </PreviewPanel>
        <button
          v-else
          type="button"
          class="path-link"
          :title="stripWindowsExtendedPrefix((row as PixivImageState).absPath)"
          @click.stop="openLocation((row as PixivImageState).absPath)"
        >
          {{ (row as PixivImageState).fileName }}
        </button>
      </template>

      <!--
        缩略图列：直接 convertFileSrc(absPath)，配合 loading="lazy" + 虚拟滚动，
        只有可见行才会真正加载。alt="" 让浏览器在加载失败时不渲染 broken icon。
      -->
      <template #image="{ row }">
        <div class="thumb-wrap">
          <img
            :src="convertFileSrc(stripWindowsExtendedPrefix((row as PixivImageState).absPath))"
            class="thumb"
            alt=""
            loading="lazy"
          />
        </div>
      </template>

      <template #status="{ row }">
        <span v-if="(row as PixivImageState).status === 'pending'" class="status pending">…</span>
        <span v-else-if="(row as PixivImageState).status === 'ok'" class="status ok" title="拉取成功">✓</span>
        <button
          v-else
          type="button"
          class="retry-btn"
          :title="(row as PixivImageState).error ?? '点击重试'"
          @click="retryRow((row as PixivImageState).pid)"
        >
          重试
        </button>
      </template>

      <!--
        Tag 气泡列：所有 tag 在 cell 里 flex-wrap 自动换行；行高由 VirtualTable 的
        autoRowHeight 模式实测每行内容决定，所以气泡墙能自然撑高、不需要内部滚动。
        当前已移动到的 tag 会被高亮 + 置灰（再点一次只会得到 ` (1)` 后缀的副本，没意义）。

        v-for 直接读 panel 级 ref `tagsByPid`：`watchEffect` 在
        `pixivUseTranslation` / `pixivExcludedTags` / 行的 `tags + translations`
        任一变更时立刻重建整张 Map，模板使用 `tagsByPid.value`（自动 unwrap）
        响应 ref 整体替换。`:key` 用 `original`：toggle 切换时原 tag 不变 →
        key 不变 → Vue 复用 DOM、只更文本 / disabled 状态。
        click / 高亮 / title 都用 `display`（当前展示出的字符串，开 toggle 时是 en）。
      -->
      <template #tags="{ row }">
        <div class="tag-list">
          <button
            v-for="t in tagsByPid.get((row as PixivImageState).pid) || []"
            :key="t.original"
            type="button"
            class="tag-chip"
            :class="{ 'is-current': (row as PixivImageState).movedTag === t.display }"
            :disabled="!store.outputDir || moving || (row as PixivImageState).movedTag === t.display"
            :title="
              (row as PixivImageState).movedTag === t.display
                ? `已在 ${t.display}`
                : store.outputDir
                  ? `移动到 ${t.display}`
                  : '请先选择输出目录'
            "
            @click="clickTag((row as PixivImageState).pid, t.display)"
          >
            {{ t.display }}
          </button>
          <span
            v-if="(tagsByPid.get((row as PixivImageState).pid)?.length ?? 0) === 0"
            class="muted"
          >
            {{ (row as PixivImageState).status === "pending" ? "…" : "无可用标签" }}
          </span>
        </div>
      </template>

      <template #movedTag="{ row }">
        <span
          v-if="(row as PixivImageState).movedTag"
          class="moved-cell"
          :title="stripWindowsExtendedPrefix((row as PixivImageState).absPath)"
        >
          → {{ (row as PixivImageState).movedTag }}
        </span>
        <span v-else class="muted">—</span>
      </template>
    </VirtualTable>
  </Panel>
</template>

<style scoped>
.pixiv-panel {
  height: 100%;
  min-height: 0;
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2) var(--ff-space-3);
  flex-shrink: 0;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--ff-space-2);
  margin-left: auto;
}
.inline-field {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.field-label {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-secondary);
  font-weight: 500;
  white-space: nowrap;
}
.output-field {
  flex: 1 1 360px;
  min-width: 280px;
}

.status-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-bg-muted);
  color: var(--ff-text-secondary);
  font-size: var(--ff-font-sm);
  flex-shrink: 0;
}
.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--ff-text-muted);
}
.status-bar.is-running .status-dot {
  background: var(--ff-accent);
  animation: pulse 1.2s ease-in-out infinite;
}
.status-bar.is-done .status-dot {
  background: var(--ff-success);
}
@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(1.3); }
}

.hint-warn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-warning-soft);
  color: var(--ff-warning);
  font-size: var(--ff-font-sm);
  flex-shrink: 0;
}
.dot-warn {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ff-warning);
  flex-shrink: 0;
}

.pixiv-table {
  flex: 1;
  min-height: 0;
}

.path-link {
  display: block;
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  color: var(--ff-accent);
  cursor: pointer;
  font: inherit;
}
.path-link:hover {
  text-decoration: underline;
}

.status {
  display: inline-block;
  font-size: var(--ff-font-sm);
}
.status.pending {
  color: var(--ff-text-muted);
}
.status.ok {
  color: var(--ff-success);
  font-weight: 700;
}

.retry-btn {
  appearance: none;
  background: transparent;
  border: 1px solid var(--ff-border-subtle);
  color: var(--ff-text-secondary);
  border-radius: var(--ff-radius-sm);
  padding: 1px 8px;
  font-size: var(--ff-font-xs);
  cursor: pointer;
}
.retry-btn:hover {
  border-color: var(--ff-accent);
  color: var(--ff-accent);
}

/* ---- tag 列：自动换行的气泡墙（autoRowHeight 模式下,行高随这里的内容自适应） ---- */
.tag-list {
  display: flex;
  flex-wrap: wrap;
  align-content: flex-start;
  align-items: center;
  gap: 4px;
  width: 100%;
  padding: 4px 0;
  box-sizing: border-box;
}
.tag-chip {
  appearance: none;
  flex-shrink: 0;
  background: var(--ff-accent-soft);
  color: var(--ff-accent);
  border: 1px solid transparent;
  border-radius: 999px;
  padding: 1px 10px;
  font-size: var(--ff-font-xs);
  line-height: 1.6;
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.12s, color 0.12s, border-color 0.12s;
}
.tag-chip:hover:not(:disabled) {
  background: var(--ff-accent);
  color: var(--ff-bg-panel);
}
.tag-chip:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
/* 当前所在 tag：高亮成"已选中"色调；disabled 状态下也保持视觉强调 */
.tag-chip.is-current {
  background: var(--ff-success-soft, var(--ff-accent-soft));
  color: var(--ff-success, var(--ff-accent));
  border-color: var(--ff-success, var(--ff-accent));
  opacity: 1;
}

/* ---- 缩略图列 ---- */
.thumb-wrap {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px 0;
  box-sizing: border-box;
}
.thumb {
  /*
   * autoRowHeight 模式下行高由 cell 内容决定:这里给一个 80px 的 max-height 上限,
   * 防止超大图把整行撑得过高;tag 列通常更高,行高最终由 tag 墙决定。
   */
  max-width: 100%;
  max-height: 80px;
  object-fit: contain;
  border-radius: 4px;
  background: var(--ff-bg-muted);
  /* 加载失败 / 还在 lazy 加载时占位框仍占据原位置 */
  min-width: 24px;
  min-height: 24px;
}

.moved-cell {
  color: var(--ff-success);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: inline-block;
  max-width: 100%;
}
.muted {
  color: var(--ff-text-muted);
  font-size: var(--ff-font-sm);
}
</style>
