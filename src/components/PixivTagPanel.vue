<script setup lang="ts">
/**
 * Pixiv 标签整理面板。
 *
 * 流程：
 * 1. 用户在任务输入侧边栏添加图片目录；
 * 2. 在本面板顶部填输出目录、点"开始扫描"；
 * 3. 后端先同步扫出所有含 PID 的图片建出 pending 行，再启动长任务并发拉 tag；
 * 4. 行上的"标签"单元格里所有 tag 都渲染成气泡；点气泡 → 把图移到 `<output>/<tag>/`；
 * 5. "已移动到"列显示该行最近一次被移到的 tag；当前所在 tag 的气泡会高亮 + 置灰。
 *
 * 设计选择（这版）：
 * - **同 PID 多行**：候选扫描出多张图共用同一 PID（作品 p0..pN）很常见。store 用
 *   `_pidIndex: Map<string, number[]>` 把每个 PID 映射到所有索引，partial 一到达
 *   就把结果应用到整组同 PID 行，每一张图都能看到打勾、都能独立点 chip 移动；
 *   行级操作（移动 / 重试 / 译名切换）按 `absPath` 定位（PID 此时不再是单行主键）。
 * - **行高随内容自适应**：tag-list 用 `flex-wrap: wrap` 自动换行；VirtualTable 开启
 *   `autoRowHeight` 后每行高度由 cell 内容（即气泡墙 + 缩略图）实测决定，不再需要
 *   单元格内部滚动条，所有 tag 都能直接看到；
 * - **可选缩略图列**：开关控制；开启后图片列默认排在最前面，且 fileName 列上的
 *   hover 浮窗会被去掉（缩略图常驻就够了）；
 * - **列自定义开**：传 `column-config-key="pixiv:tags"`，用户可隐藏列 / 调整顺序 /
 *   切换左固定，配置随 localStorage 持久化；
 * - **fit-width 开**：列总宽不大，让 tag 列吃掉剩余空间最舒服；
 * - **全局译名开关 + 行级覆盖**：与配置中心同步 `pixivUseTranslation` 是默认值，
 *   每行右侧"译名"列暴露一个 segmented 控件（`global` / `translated` / `original`）
 *   可对该行单独覆盖。每行 chip 的展示走 **render-time 函数 `displayTagsForRow`**：
 *   每次模板渲染时同步访问 `pixivUseTranslation` / `pixivLocalTagTranslations` /
 *   `row.tags` / `row.translations` / `row.useTranslationOverride`，依赖直接落在组件 render effect 上，依赖变化
 *   一定触发重渲染——绕过中间 ref / computed 任何潜在的缓存 quirks（之前用
 *   `watchEffect + ref` 时遇到过翻译开关切了 chip 不刷新的灰区）。VirtualTable
 *   的 v-for 只对可见行执行，per-row 计算在屏内十几行上跑，比"全表 watchEffect
 *   重算 + 整体替换 ref"更省；
 * - **节流刷新**：partial 进入 store 缓冲区后按 `pixivPartialFlushIntervalMs` 配置
 *   决定立刻 commit（默认 0）还是按间隔批量 commit。50K 张图配 500ms 节流明显
 *   降低 UI 抖动；
 * - **后端日志**：每条 PID 完成（成功 / 失败）后端都会发一行 task_log，配合日志
 *   面板能看到具体哪个 ID 出错；前端"重试失败"按钮一键串行重试所有 error 行。
 */
import { computed, onMounted, ref, watch } from "vue";
import { useStorage } from "@vueuse/core";
import { ElMessage } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Folder, RefreshRight } from "@element-plus/icons-vue";

import { usePixivTagStore } from "../stores/pixivTag";
import { useConfigStore } from "../stores/config";
import { stripWindowsExtendedPrefix } from "../utils/path";
import { DEFAULT_EXTREME_ROW_THRESHOLD, EXTREME_OVERSCAN, NORMAL_OVERSCAN } from "../constants/task";
import type { PixivImageState, PixivTranslationOverride } from "../types/pixivTag";
import type { VirtualColumn } from "../types/virtualTable";
import Panel from "./common/Panel.vue";
import PathPreviewLink from "./PathPreviewLink.vue";
import VirtualTable from "./common/VirtualTable.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = usePixivTagStore();
const configStore = useConfigStore();

const moving = ref(false);
const retryingAll = ref(false);

/**
 * 是否在表里显示图片缩略图列。开启后会：
 * 1) 在所有列**最前面**插一列 96px 宽的"图片"，cell 渲染 `convertFileSrc(absPath)`；
 * 2) 把"文件"列上的 PreviewPanel hover 浮窗去掉——既然缩略图常驻，再悬浮一遍是浪费。
 *
 * 跨会话持久化在 localStorage（`pixivTag:showImage`）。
 */
const showImageColumn = useStorage<boolean>("pixivTag:showImage", false);

/**
 * 计算单行 chip 列展示数据：`[{ original, display }]`。
 *
 * 这里**不用 panel 级 computed / watchEffect**，而是改成 **render-time 函数**。
 * 之前用 `watchEffect + ref` 写过两版，都遇到过"翻译开关切了 chip 不刷新"的现象，
 * 怀疑根因是 Pinia 的嵌套 reactive proxy + Map 输出 + partial 多次写入下，
 * effect 的依赖图在某个时间点漏掉了 row.translations 的依赖（lazy 缓存命中
 * 旧值不重算）。
 *
 * **render-time 函数的好处**：每次组件渲染（v-for / slot 求值）时都会**同步**
 * 调用本函数，访问 `configStore.settings.pixivUseTranslation` /
 * `configStore.settings.pixivLocalTagTranslations` / `row.tags` /
 * `row.translations` / `row.useTranslationOverride` 都直接发生在组件 render
 * effect 的栈上，依赖一定建立、一定能在依赖变化时触发组件重渲染——绕过中间
 * 任何 ref / computed 的缓存层。
 *
 * 性能：VirtualTable 只对**可见行**调用 v-for 槽，所以这里是 per-visible-row
 * 计算，不是全表。50K 张图也只在屏幕里十几行上算，比 panel 级 watchEffect
 * "全表重算然后 ref 整体替换"更省。
 *
 * 数据流（对照用户给的 json.json 示例）：
 * - 后端从 `body.tags.tags[*]` 取 `tag`（原标签字符串），同时如果 `translation.en`
 *   存在且非空，把 `(tag → en)` 加进 translations Map。最终 PIXIV partial item 携带
 *   `tags: ["コイカツ", "キャラ配布(コイカツ)", ...]` 与 `translations: {"コイカツ":
 *   "恋活", "キャラ配布(コイカツ)": "人物卡（恋活）", ...}`。
 * - 前端 store 把它们写到 row.tags / row.translations。
 * - 这里按 row.useTranslationOverride（行级覆盖）or 全局 `pixivUseTranslation`
 *   决定 useT；useT = true 时先取本地翻译表 `pixivLocalTagTranslations[tag]`，
 *   没有本地译名再取 Pixiv 响应里的 `translations[tag]`，都没有则回落原 tag。
 * - 排除项始终先匹配原 tag，确保原 tag 被排除时切到译名也不会露出；同时兼容
 *   匹配当前展示字符串，保留此前"按译名排除"的用法。
 */
function displayTagsForRow(row: PixivImageState): { original: string; display: string }[] {
  // 显式读所有依赖 —— 即便函数体后续没用到（useT = false 时不读 trans[t]），
  // 这里"先读一下"也确保依赖建立。组件 render effect 的依赖追踪是按"实际访问"
  // 来的，所以提前用变量接住每个 reactive 字段就不会出现"useT = false 那次没追踪"
  // 的灰区。
  const globalUseT = Boolean(configStore.settings.pixivUseTranslation);
  const excluded = configStore.settings.pixivExcludedTags ?? [];
  const ex = new Set(excluded);
  const localTranslations = configStore.settings.pixivLocalTagTranslations ?? {};
  const tags = row.tags;
  const translations = row.translations;
  const override: PixivTranslationOverride = row.useTranslationOverride ?? "global";

  const useT =
    override === "translated"
      ? true
      : override === "original"
        ? false
        : globalUseT;

  const trans = translations || {};
  const items: { original: string; display: string }[] = [];
  for (const t of tags) {
    const excludedByOriginal = ex.has(t);
    let display = t;
    if (useT) {
      const local = localTranslations[t];
      const remote = trans[t];
      if (typeof local === "string" && local.length > 0) {
        display = local;
      } else if (typeof remote === "string" && remote.length > 0) {
        display = remote;
      }
    }
    if (excludedByOriginal || ex.has(display)) continue;
    items.push({ original: t, display });
  }
  items.sort((a, b) => a.display.localeCompare(b.display));
  return items;
}

const isExtreme = computed(() => {
  const th = configStore.settings.extremeRowThreshold || DEFAULT_EXTREME_ROW_THRESHOLD;
  return store.rows.length > th;
});

/**
 * 列定义。**图片列在最前**（用户开启缩略图后，最直接的视觉就在最左侧）。
 * 列自定义会持久化到 localStorage（key = `vtable:col:pixiv:tags`），用户可调整。
 *
 * 排序约定：image > fileName > pid > status > tags > movedTag > override。
 * 译名切换列放最右，跟"已移动到"一样属于行级元数据，避免抢眼挤掉 tag 列宽度。
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
    { key: "movedTag", label: "已移动到", width: 160, slotName: "movedTag", resizable: true },
    { key: "override", label: "译名", width: 178, slotName: "override" }
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

/**
 * 当前刷新间隔的可读描述，给用户一个明确预期：默认 0 时显示"实时"，
 * 否则显示"<ms>ms 节流"。
 */
const flushIntervalLabel = computed(() => {
  const v = configStore.settings.pixivPartialFlushIntervalMs;
  if (typeof v !== "number" || !Number.isFinite(v) || v <= 0) return "实时";
  return `${Math.min(10000, Math.floor(v))}ms 节流`;
});

/**
 * 传给 VirtualTable 的 slot 刷新键。
 *
 * tag 气泡 slot 除了 row 本身，还依赖全局译名开关、排除项与本地翻译表；这些状态变化时
 * row 引用不会变。把它们折成一个轻量 key 传下去，确保虚拟表复用可见行 DOM
 * 时也会重建 slot cell，立刻把 "キャラ配布(コイカツ)" 切成 "人物卡（恋活）"。
 */
const tagSlotRefreshKey = computed(() => {
  const excluded = configStore.settings.pixivExcludedTags ?? [];
  const localTranslations = configStore.settings.pixivLocalTagTranslations ?? {};
  return [
    configStore.settings.pixivUseTranslation ? "t" : "o",
    excluded.join("\u001f"),
    JSON.stringify(localTranslations)
  ].join("|");
});

const overrideOptions: { label: string; value: PixivTranslationOverride; tip: string }[] = [
  { label: "全局", value: "global", tip: "跟随顶部的「使用英文译名」开关" },
  { label: "原 tag", value: "original", tip: "本行强制显示原始 tag" },
  { label: "译名", value: "translated", tip: "本行强制显示英文译名（缺译名仍回落原 tag）" }
];

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

/** 单行重试：用 absPath 定位（不能用 pid，同 PID 多行时 pid 不唯一）。 */
async function retryRow(absPath: string) {
  try {
    await store.retry(absPath);
  } catch (e) {
    ElMessage.error(`重试失败：${String(e)}`);
  }
}

/**
 * 一键重试所有失败行。store 内部按 PID 去重 + 串行重试，避免一次性发起几百条
 * HTTP 触发限流。重试期间按钮自旋禁用。
 */
async function retryAllFailed() {
  if (retryingAll.value) return;
  if (store.errorCount === 0) {
    ElMessage.info("没有失败的行可重试");
    return;
  }
  retryingAll.value = true;
  try {
    const { tried, failed } = await store.retryFailed();
    if (failed === 0) {
      ElMessage.success(`已重试 ${tried} 个 PID，全部成功`);
    } else {
      ElMessage.warning(`已重试 ${tried} 个 PID，仍有 ${failed} 个失败`);
    }
  } catch (e) {
    ElMessage.error(`批量重试出错：${String(e)}`);
  } finally {
    retryingAll.value = false;
  }
}

/**
 * 点击 tag 气泡：把 `<row.absPath>` 移到 `<outputDir>/<tag>/`。
 *
 * `tag` 参数是模板里 `t.display` —— 当前行该显示的字符串（按 row 级 useT 判定）。
 * 后端按这个字符串建子目录，所以 `movedTag` 自然记录的就是"实际落盘的目录名"。
 */
async function clickTag(absPath: string, tag: string) {
  if (!store.outputDir) {
    ElMessage.warning("请先选择输出目录");
    return;
  }
  if (moving.value) return; // 简单避免连点
  moving.value = true;
  try {
    await store.moveByTag(absPath, tag);
    ElMessage.success(`已移动到 ${tag}`);
  } catch (e) {
    ElMessage.error(`移动失败：${String(e)}`);
  } finally {
    moving.value = false;
  }
}

function changeOverride(absPath: string, override: PixivTranslationOverride) {
  store.setRowTranslationOverride(absPath, override);
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
          配置中心和这里改一处两处都生效。每行右侧"译名"列可单独覆盖。
        -->
        <span class="field-label">使用英文译名</span>
        <el-switch v-model="configStore.settings.pixivUseTranslation" />
      </label>

      <span class="meta-tip" :title="`刷新策略可在 设置 → Pixiv 标签整理 中调整`">
        刷新：{{ flushIntervalLabel }}
      </span>

      <div class="actions">
        <el-button
          plain
          :icon="RefreshRight"
          :loading="retryingAll"
          :disabled="store.running || store.errorCount === 0"
          @click="retryAllFailed"
        >
          重试失败 ({{ store.errorCount }})
        </el-button>
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

    <!--
      :key 用 absPath：同 PID 多行时 PID 不唯一，用它做 row-key 会让 VirtualTable
      把多行视作"同一行"互相覆盖，进而 selection / autoRowHeight 测量都会错乱。
      absPath 是行的稳定主键。
    -->
    <VirtualTable
      :key="showImageColumn ? 'pixiv-with-image' : 'pixiv-no-image'"
      class="pixiv-table"
      :rows="store.rows"
      :columns="columns"
      :item-height="36"
      auto-row-height
      :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
      row-key="absPath"
      column-config-key="pixiv:tags"
      :slot-refresh-key="tagSlotRefreshKey"
      fit-width
      empty-text="尚无数据，点'开始扫描'识别 Pixiv 图片"
    >
      <template #fileName="{ row }">
        <PathPreviewLink
          v-if="!showImageColumn"
          :path="(row as PixivImageState).absPath"
          :label="(row as PixivImageState).fileName"
        />
        <PathPreviewLink
          v-else
          :path="(row as PixivImageState).absPath"
          :label="(row as PixivImageState).fileName"
          :preview="false"
        />
      </template>

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
          @click="retryRow((row as PixivImageState).absPath)"
        >
          重试
        </button>
      </template>

      <!--
        Tag 气泡列：所有 tag 在 cell 里 flex-wrap 自动换行；行高由 VirtualTable 的
        autoRowHeight 模式实测每行内容决定，所以气泡墙能自然撑高、不需要内部滚动。
        当前已移动到的 tag 会被高亮 + 置灰（再点一次只会得到 ` (1)` 后缀的副本）。

        v-for 直接调 panel 内的 `displayTagsForRow(row)` —— render-time 函数路线，
        每次渲染时同步访问 reactive，依赖一定建立、依赖变化时一定重渲染。
        `:key` 用 `original`：toggle 切换时原 tag 不变 → key 不变 → Vue 复用 DOM、
        只更文本 / disabled 状态。click / 高亮 / title 都用 `display`（当前展示的字符串）。
      -->
      <template #tags="{ row }">
        <div class="tag-list">
          <template v-for="t in displayTagsForRow(row as PixivImageState)" :key="t.original">
            <button
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
              @click="clickTag((row as PixivImageState).absPath, t.display)"
            >
              {{ t.display }}
            </button>
          </template>
          <span
            v-if="displayTagsForRow(row as PixivImageState).length === 0"
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

      <!--
        译名覆盖列：segmented 三态控件覆盖单行的译名行为。改值立刻触发上面
        watchEffect 重算 tagsByAbsPath，那一行的 chip 文本同步刷新。
      -->
      <template #override="{ row }">
        <div class="override-seg">
          <button
            v-for="opt in overrideOptions"
            :key="opt.value"
            type="button"
            class="seg-btn"
            :class="{ 'is-active': ((row as PixivImageState).useTranslationOverride ?? 'global') === opt.value }"
            :title="opt.tip"
            @click="changeOverride((row as PixivImageState).absPath, opt.value)"
          >
            {{ opt.label }}
          </button>
        </div>
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
.meta-tip {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  background: var(--ff-bg-muted);
  padding: 2px 8px;
  border-radius: 999px;
  white-space: nowrap;
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
  max-width: 100%;
  max-height: 80px;
  object-fit: contain;
  border-radius: 4px;
  background: var(--ff-bg-muted);
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

/* ---- 译名覆盖列：自有 segmented 控件 ---- */
.override-seg {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-sm);
  background: var(--ff-bg-panel);
  overflow: hidden;
}
.seg-btn {
  appearance: none;
  background: transparent;
  border: 0;
  border-right: 1px solid var(--ff-border-subtle);
  padding: 2px 8px;
  font-size: var(--ff-font-xs);
  color: var(--ff-text-secondary);
  cursor: pointer;
  white-space: nowrap;
  line-height: 1.6;
  transition: background 0.12s, color 0.12s;
}
.seg-btn:last-child {
  border-right: 0;
}
.seg-btn:hover {
  background: var(--ff-bg-muted);
}
.seg-btn.is-active {
  background: var(--ff-accent);
  color: var(--ff-bg-panel);
  font-weight: 600;
}
</style>
