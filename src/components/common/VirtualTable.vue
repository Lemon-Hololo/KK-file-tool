<script setup lang="ts">
/**
 * 通用虚拟表格。
 *
 * 行固定高、列宽可拖拽、支持全选 / 分页 / 固定列 / ellipsis + tooltip / 列自定义。
 * 日志等动辄上万条的场景用此组件替代 `el-table`。
 *
 * # 关键约定
 * - `rowKey` 必填或行带稳定字段（`oldPath` / `absPath`）；找不到稳定键时退到行索引，
 *   避免 `Math.random()` 导致的 key 抖动。
 * - `formatter` 若昂贵，调用方自己 memo；本组件会尽量只调用一次。
 *
 * # 单一滚动容器
 * 整个表格内部只有一个滚动容器 `.vtable-scroll`，同时承担横向与纵向滚动。
 * 这样避免了"外层横滚 + 内层纵滚"嵌套带来的三个顽疾：
 * - 滚动条位置与实际内容不同步（到底了但滑块没到底，从底部往上拉也拉不动）
 * - 固定列 `position: sticky; left: X` 在 body 行里失效（sticky 找到的 scroll
 *   ancestor 是内层纵滚容器，而它并不横滚，于是固定列不响应横滚）
 * - 外层与内层各自出一条横滚条，视觉上很脏
 *
 * 虚拟滚动不走 VueUse 的 `useVirtualList`——它自带 `overflow-y: auto` 的容器 +
 * `marginTop` 偏移 wrapper 的实现方式，和单滚动容器需求冲突，也是造成滚动条错位
 * 的元凶。这里自己写：监听 `.vtable-scroll.scroll`（raf 节流），按 `scrollTop /
 * itemHeight` 算可见区间，可见行 `position: absolute; top: N*itemHeight` 放进
 * `height = totalRows*itemHeight` 的占位 `.body` 里。
 *
 * # 宽度
 * `.vtable { width: 100% }` 始终撑满父容器。内容层 `.vtable-content` 显式 `width:
 * totalColumnWidth` 并用 `min-width: 100%` 兜底：
 * - 列总宽 ≤ 容器：内容层被 `min-width` 拉伸到容器宽，表头/行 `width: 100%` 跟随拉伸，
 *   最后一列右侧的留白用表头底色 / 行底色填充，不再露出外层容器底色那一条空白。
 * - 列总宽 > 容器：内容层按 `totalColumnWidth` 展开，由单滚动容器横滚。
 *
 * # 高度
 * 两种模式二选一：
 * - 固定高度：传 `height`（px），`.vtable-scroll` 用该值作为 inline height。
 * - 自适应（auto-height）：省略 `height`，`.vtable` 与 `.vtable-scroll` 都用
 *   `flex: 1; min-height: 0` 撑满父容器。**此模式要求父链路径上一路都是
 *   `display: flex; flex-direction: column` 且 `min-height: 0`**，不然 flex item
 *   不会正确分配剩余空间，会塌缩或溢出。OpsPanel / ModScanPanel 采用此模式，
 *   就不必再算 `panelHeight - 180` 这种魔数去挤出一个 px 值。
 *
 * # 行高
 * 默认每行固定 `itemHeight` px——简单且支持最快的虚拟滚动数学（`scrollTop /
 * itemHeight` 直接拿首行索引）。
 *
 * 开启 `autoRowHeight` 后改成"内容驱动行高 + ResizeObserver 测量 + 前缀和定位"：
 * - 渲染层不再写 `height`，只写 `min-height: itemHeight`；行的实际高度由内部
 *   cell 决定（典型场景：带气泡墙 / 缩略图的列）。
 * - 每个 row 元素被一个 `ResizeObserver` 观察，测得高度按 rowKey 写进
 *   `measuredRowHeights`；测量更新走 rAF 批量提交，避免一帧重算多次。
 * - 测得高度变化后，前缀和 `rowOffsets` 重算；`visibleRange` 用二分查找拿到
 *   首/末可见行，每行的 `top` 取 `offsets[index]` 而不是 `index * itemHeight`。
 * - 未测量的行用 `itemHeight` 兜底——所以 `itemHeight` 在此模式下相当于"行
 *   预估高度"，传一个接近典型行的值能让滚动条少抖几下。
 *
 * 这套是为"每行 cell 内容高度差异很大"的场景准备的（Pixiv tag 列里有的 5 个 chip
 * 一行、有的 80 个 chip 五行——固定行高要么截掉看不到，要么大部分行留白浪费空间）。
 * 数据均匀的常规场景仍然走默认固定高度，数学最简单也最快。
 *
 * # 列自定义
 * 开启 `columnConfigurable`（默认 true）后，`.vtable-toolbar` 右端有"列设置"按钮，
 * 用户可在弹出面板里：
 * - 勾选/取消列的显示
 * - 勾选/取消列的左固定
 * - 拖拽调整列顺序
 *
 * 列顺序拖拽用 **pointer 事件**而不是 HTML5 DnD：Tauri WebView 会拦截 HTML5
 * 拖拽（因为它要处理外部文件拖入），导致拖拽时光标变成"禁止"、无法 drop。
 * pointer 事件是底层 DOM 事件，Tauri 不会拦截。
 *
 * selection 列（`__select__`）不受自定义影响：永远是第一列、永远左固定。
 *
 * **固定列连续前缀约束**：除 selection 外，左固定列必须是紧跟 selection 的连续前缀，
 * 中间不允许有非固定列。任何操作后都通过 `normalizeFixedContiguity` 兜底校正。
 *
 * 传 `columnConfigKey` 可把配置持久化到 localStorage（`vtable:col:<key>`）；
 * 不传则仅会话内生效。
 */

import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useStorage } from "@vueuse/core";
import { ElMessage } from "element-plus";
import { Setting } from "@element-plus/icons-vue";
import { copyText } from "../../utils/clipboard";
import type {
  PaginationConfig,
  RenderColumn,
  VirtualColumn,
  VirtualColumnState
} from "../../types/virtualTable";

/** 表头高度（CSS 里同步写死 36px）。用于计算可见行数。 */
const HEADER_HEIGHT = 36;

/**
 * Windows WebView2（Tauri 默认）下的横向滚动条高度兜底值。
 *
 * 浏览器 `overflow: auto` 的判断是"内容超过 clientHeight 才出纵向条"，但 clientHeight
 * 已经被横向滚动条挤掉了 ~15–17px。所以当横向滚动条出现时，原本"刚好装下"的内容
 * 会被挤压触发不必要的纵向滚动条。判断"是否真需要纵向滚动"时把这部分加回去。
 *
 * Tauri 桌面用 Windows WebView2，滚动条占空间且固定 17px；如果哪天平台扩展到 macOS
 * 的 floating scrollbar 系统，这里偏保守不会误判（因为 floating 模式下 clientHeight
 * 不会被横向条挤）。
 */
const HORIZONTAL_SCROLLBAR_ALLOWANCE = 17;

const props = withDefaults(
  defineProps<{
    rows: any[];
    columns: VirtualColumn[];
    /**
     * 滚动容器的固定高度（px）。
     * 省略时进入 auto-height 模式：`.vtable` 以 `flex: 1` 撑满父容器，
     * 父容器必须是 `display: flex; flex-direction: column`。
     */
    height?: number;
    itemHeight?: number;
    overscan?: number;
    rowKey?: string | ((row: any) => string | number);
    selectable?: boolean;
    pagination?: PaginationConfig;
    /** 空行时显示的占位文案。 */
    emptyText?: string;
    /** 列配置持久化键；传了就按 `vtable:col:<key>` 存 localStorage，不传则仅内存保存。 */
    columnConfigKey?: string;
    /** 是否暴露"列设置"按钮；默认 true。 */
    columnConfigurable?: boolean;
    /** 是否允许双击复制单元格文本。默认 true。 */
    copyable?: boolean;
    /**
     * 列宽按容器宽度自适应：开启后总列宽永远等于容器宽度，消除横滚。
     *
     * 伸缩权重 = `width - minWidth`；权重为 0 的列（只声明 `width` 或 `width === minWidth`）
     * 视为固定列，不参与分配。所有列的 `minWidth` 总和仍大于容器时只能出横滚，
     * 此时回退到原始 `colWidths`（拖拽值）保留可读性。
     */
    fitWidth?: boolean;
    /**
     * 行高随内容自适应：开启后行高由 cell 内容决定，单元格不再需要内部滚动条。
     * 实现见组件顶部 `# 行高` 一节。默认 false——按 itemHeight 走固定行高的传统模式。
     */
    autoRowHeight?: boolean;
  }>(),
  {
    itemHeight: 36,
    overscan: 10,
    rowKey: "id",
    selectable: false,
    emptyText: "暂无数据",
    columnConfigurable: true,
    fitWidth: false,
    autoRowHeight: false,
    copyable: true
  }
);

/** 未传 `height` 时进入 auto-height 模式，由父 flex column 决定容器高度。 */
const autoHeight = computed(() => props.height == null);

const emit = defineEmits<{
  (e: "rowClick", row: any): void;
  (e: "selectionChange", rows: any[]): void;
  (e: "pageChange", page: number): void;
  (e: "pageSizeChange", pageSize: number): void;
}>();

// ---- 选择 ----
const selectedSet = ref<Set<string | number>>(new Set());

/**
 * 取行稳定主键。找不到时退到 `__idx:{n}`，避免 `Math.random()` 造成
 * 每次渲染 key 漂移（破坏 v-for 复用与选择状态）。
 */
function getRowKey(row: any, index = -1): string | number {
  if (typeof props.rowKey === "function") return props.rowKey(row);
  const k = row?.[props.rowKey] ?? row?.oldPath ?? row?.absPath ?? row?.itemId;
  return k != null ? k : `__idx:${index}`;
}

watch(
  () => props.rows,
  () => {
    // 只关心引用变化；内部字段变化不应重建 selection（避免大数据量下昂贵 diff）。
    const keys = new Set(props.rows.map((r, i) => getRowKey(r, i)));
    let changed = false;
    const next = new Set<string | number>();
    for (const k of selectedSet.value) {
      if (keys.has(k)) next.add(k);
      else changed = true;
    }
    if (changed) {
      selectedSet.value = next;
      emitSelection();
    }
  }
);

const isAllSelected = computed(() => {
  const rows = pagedRows.value;
  return rows.length > 0 && rows.every((r, i) => selectedSet.value.has(getRowKey(r, i)));
});

const isIndeterminate = computed(() => {
  const rows = pagedRows.value;
  const count = rows.filter((r, i) => selectedSet.value.has(getRowKey(r, i))).length;
  return count > 0 && count < rows.length;
});

function toggleAll(checked: boolean) {
  const rows = pagedRows.value;
  if (checked) rows.forEach((r, i) => selectedSet.value.add(getRowKey(r, i)));
  else rows.forEach((r, i) => selectedSet.value.delete(getRowKey(r, i)));
  emitSelection();
}

function toggleRow(row: any, index: number, checked: boolean) {
  const key = getRowKey(row, index);
  if (checked) selectedSet.value.add(key);
  else selectedSet.value.delete(key);
  emitSelection();
}

function emitSelection() {
  const rowMap = new Map(props.rows.map((r, i) => [getRowKey(r, i), r]));
  const rows = Array.from(selectedSet.value)
    .map((k) => rowMap.get(k))
    .filter(Boolean);
  emit("selectionChange", rows);
}

// ---- 分页 ----
const internalPage = ref(props.pagination?.page || 1);
const internalPageSize = ref(props.pagination?.pageSize || 20);

watch(
  () => props.pagination?.page,
  (v) => {
    if (typeof v === "number") internalPage.value = v;
  }
);
watch(
  () => props.pagination?.pageSize,
  (v) => {
    if (typeof v === "number") internalPageSize.value = v;
  }
);

const paginationMode = computed(() => props.pagination?.mode || "client");
const showPagination = computed(() => !!props.pagination?.show);

const pagedRows = computed(() => {
  if (!showPagination.value) return props.rows;
  if (paginationMode.value === "server") return props.rows;
  const start = (internalPage.value - 1) * internalPageSize.value;
  return props.rows.slice(start, start + internalPageSize.value);
});

const totalRows = computed(() => {
  if (!showPagination.value) return props.rows.length;
  return paginationMode.value === "server"
    ? props.pagination?.total ?? props.rows.length
    : props.rows.length;
});

// ---- 列宽 / 拖拽 ----
const colWidths = reactive<Record<string, number>>({});

watch(
  () => props.columns,
  (cols) => {
    for (const c of cols) {
      if (!colWidths[c.key]) colWidths[c.key] = c.width || c.minWidth || 120;
    }
  },
  { immediate: true, deep: true }
);

// ---- 列用户配置（显示 / 左固定 / 顺序） ----

/** 根据 `props.columns` 生成一份默认配置。 */
function buildDefaultStates(cols: VirtualColumn[]): VirtualColumnState[] {
  return cols.map((c, i) => ({
    key: c.key,
    visible: true,
    fixed: c.fixed === "left",
    order: i
  }));
}

/**
 * 列状态存储：有 `columnConfigKey` 用 `useStorage` 持久化到 localStorage，
 * 否则纯内存 `ref`。两种形态接口一致（`.value` 读写整个数组）。
 */
const columnStates = props.columnConfigKey
  ? useStorage<VirtualColumnState[]>(
      `vtable:col:${props.columnConfigKey}`,
      buildDefaultStates(props.columns),
      undefined,
      { mergeDefaults: false }
    )
  : ref<VirtualColumnState[]>(buildDefaultStates(props.columns));

/**
 * 同步 `props.columns` 与 `columnStates`：被删除的列从配置剔除；新增列按它们在
 * `props.columns` 里的"源顺序"插入到现有 saved order 的合适位置 —— 而不是无脑
 * append 到最后。
 *
 * 为什么不 append：调用方在 `props.columns` 里给某列写"应该在第 1 位"是它对默认
 * 顺序的声明。如果用户已经持久化过列顺序、然后调用方又 toggle 出一个本应在前
 * 的新列（例如 PixivTagPanel 开启缩略图列后图片列应当排在最前），append 到末尾
 * 会和声明意图相反。
 *
 * 算法：对每个"saved 里没有"的源列 c，在当前 saved order（按 `order` 升序）中
 * 找到第一条"源索引比 c 大"的列 next；把 c 插到 next 前面。如果不存在（即 c 在
 * 源里位置最靠右），就 push 到末尾。已存在的列保留用户改过的 visible/fixed/order。
 *
 * 同步后立刻校正固定连续性，避免持久化残留的脏状态。
 */
watch(
  () => props.columns.map((c) => c.key).join("|"),
  () => {
    const sourceIndex = new Map<string, number>();
    props.columns.forEach((c, i) => sourceIndex.set(c.key, i));

    const curKeys = new Set(sourceIndex.keys());
    // 现有的 saved 状态按 order 升序排列；只保留还在 props.columns 里的列。
    const kept = columnStates.value
      .filter((s) => curKeys.has(s.key))
      .sort((a, b) => a.order - b.order);
    const known = new Set(kept.map((s) => s.key));

    // 新列：仍按 props.columns 的源顺序遍历，逐个找插入位置。
    for (const c of props.columns) {
      if (known.has(c.key)) continue;
      const newSrcIdx = sourceIndex.get(c.key)!;
      // kept 里第一条源索引大于当前列的，就是右边邻居 —— 插它前面。
      const insertAt = kept.findIndex(
        (s) => (sourceIndex.get(s.key) ?? Number.MAX_SAFE_INTEGER) > newSrcIdx
      );
      const fresh = {
        key: c.key,
        visible: true,
        fixed: c.fixed === "left",
        order: 0
      };
      if (insertAt < 0) {
        kept.push(fresh);
      } else {
        kept.splice(insertAt, 0, fresh);
      }
      known.add(c.key);
    }

    // order 重新按数组下标顺序写一遍，外部消费方不应该再依赖原始的 order 数值。
    kept.forEach((s, i) => (s.order = i));
    columnStates.value = kept;
    normalizeFixedContiguity(columnStates.value);
  },
  { immediate: true }
);

/**
 * 兜底规范化：固定列必须是紧跟 selection 的连续前缀，中间不允许出现非固定列。
 * 隐藏列不参与约束（被跳过，不会把它们当作"断链点"）。
 */
function normalizeFixedContiguity(states: VirtualColumnState[]) {
  const ordered = [...states].sort((a, b) => a.order - b.order);
  let seenUnfixed = false;
  for (const s of ordered) {
    if (!s.visible) continue;
    if (!s.fixed) {
      seenUnfixed = true;
      continue;
    }
    if (seenUnfixed) s.fixed = false;
  }
}

/**
 * 用户勾选/取消某列的固定状态。
 *
 * - 勾选：把"按顺序位于该列左侧的所有可见列"一并标记为固定（前缀闭合）
 * - 取消：把"按顺序位于该列及右侧的所有可见列"一并解除固定（后缀解除）
 */
function setFixed(key: string, fixed: boolean) {
  const ordered = [...columnStates.value].sort((a, b) => a.order - b.order);
  const visibleOrdered = ordered.filter((s) => s.visible);
  const idx = visibleOrdered.findIndex((s) => s.key === key);
  if (idx < 0) return;
  if (fixed) {
    for (let j = 0; j <= idx; j++) visibleOrdered[j].fixed = true;
  } else {
    for (let j = idx; j < visibleOrdered.length; j++) visibleOrdered[j].fixed = false;
  }
  normalizeFixedContiguity(columnStates.value);
  columnStates.value = [...columnStates.value];
}

/** 切换某列的显示；隐藏时顺便解除 fixed，避免下次再显示时残留。 */
function setVisible(key: string, visible: boolean) {
  const s = columnStates.value.find((c) => c.key === key);
  if (!s) return;
  s.visible = visible;
  if (!visible && s.fixed) s.fixed = false;
  normalizeFixedContiguity(columnStates.value);
  columnStates.value = [...columnStates.value];
}

/** 把 `fromOrder` 位置的列移到 `toOrder` 位置（splice 语义）。 */
function moveColumn(fromOrder: number, toOrder: number) {
  if (fromOrder === toOrder) return;
  const ordered = [...columnStates.value].sort((a, b) => a.order - b.order);
  const [moved] = ordered.splice(fromOrder, 1);
  ordered.splice(toOrder, 0, moved);
  ordered.forEach((s, i) => (s.order = i));
  normalizeFixedContiguity(ordered);
  columnStates.value = ordered;
}

/** 恢复为 `props.columns` 定义的默认顺序 / 默认 fixed / 全部显示。 */
function resetColumnConfig() {
  columnStates.value = buildDefaultStates(props.columns);
}

/**
 * 容器宽度（由 `.vtable-scroll` 的 ResizeObserver 维护）。
 * fitWidth 关闭或尚未测量到时保持 0，此时退化为原始 `colWidths`。
 */
const containerWidth = ref(0);

/**
 * fit-width 后每列的最终宽度表。
 *
 * 算法：
 * - `base` = 当前 `colWidths[key]`（即用户拖拽后的值，或初始 `width / minWidth`）
 * - `min` = `col.minWidth ?? col.width ?? 60`（没有 `minWidth` 的列视为固定列：min === width）
 * - `weight` = `base - min`（固定列权重 0，不参与伸缩）
 * - 目标总宽 = 容器宽度；`extra = target - totalMin`，按 `(weight / totalWeight)` 分配给每列
 * - `totalMin > containerW`（所有列最小宽都塞不下）时回退原 `colWidths`，保留可读性让横滚出现
 *
 * 最后一列吃掉整数舍入的误差，避免因为 1px 差让 `.vtable-content` 超出 `.vtable-scroll`
 * 再冒一次横滚。
 */
/**
 * fit-width 是否当前能"撑满容器" —— 关键：当所有列的最小宽度之和已经超过容器宽度
 * 时，不再 fit，回退到原始 `colWidths` 让横向滚动接管（见 `.vtable-scroll--fit` 的
 * CSS 条件）。这样窄屏 + 多列的场景里用户仍然能横滑看到被挤出去的列，而不是被
 * `overflow-x: hidden` 一刀切掉。
 */
const fitWidthApplied = computed(() => {
  if (!props.fitWidth) return false;
  const containerW = containerWidth.value;
  if (!containerW) return false;

  const colByKey = new Map(props.columns.map((c) => [c.key, c]));
  let totalMin = props.selectable ? 55 : 0;
  for (const s of columnStates.value) {
    if (!s.visible) continue;
    const orig = colByKey.get(s.key);
    if (!orig) continue;
    totalMin += orig.minWidth ?? orig.width ?? 60;
  }
  return totalMin <= containerW;
});

const fittedColWidths = computed<Record<string, number>>(() => {
  if (!fitWidthApplied.value) return colWidths;
  const containerW = containerWidth.value;

  const colByKey = new Map(props.columns.map((c) => [c.key, c]));
  const visibleCols = [...columnStates.value]
    .sort((a, b) => a.order - b.order)
    .filter((s) => s.visible)
    .map((s) => {
      const orig = colByKey.get(s.key);
      if (!orig) return null;
      const base = colWidths[s.key] ?? orig.width ?? orig.minWidth ?? 120;
      const min = orig.minWidth ?? orig.width ?? 60;
      return { key: s.key, base, min };
    })
    .filter((x): x is { key: string; base: number; min: number } => !!x);

  // selection 列 base = min = 55，权重 0 永不伸缩
  const selectionCol = props.selectable
    ? [{ key: "__select__", base: 55, min: 55 }]
    : [];
  const all = [...selectionCol, ...visibleCols];

  const totalMin = all.reduce((s, c) => s + c.min, 0);
  const totalWeight = all.reduce((s, c) => s + (c.base - c.min), 0);
  const target = containerW;
  const extra = target - totalMin;

  const result: Record<string, number> = { ...colWidths };
  let acc = 0;
  for (let i = 0; i < all.length; i++) {
    const c = all[i];
    let w: number;
    if (i === all.length - 1) {
      w = Math.max(c.min, target - acc);
    } else {
      const portion = totalWeight > 0 ? (c.base - c.min) / totalWeight : 0;
      w = Math.max(c.min, Math.round(c.min + portion * extra));
      acc += w;
    }
    result[c.key] = w;
  }
  return result;
});

function normalizeColumn(c: VirtualColumn): RenderColumn {
  return {
    key: c.key,
    label: c.label,
    width: fittedColWidths.value[c.key] || c.width || c.minWidth || 120,
    ellipsis: !!c.ellipsis,
    slotName: c.slotName,
    formatter: c.formatter,
    fixed: c.fixed,
    resizable: !!c.resizable
  };
}

const renderColumns = computed<RenderColumn[]>(() => {
  const selectableCol: RenderColumn[] = props.selectable
    ? [
        {
          key: "__select__",
          label: "选择",
          width: fittedColWidths.value["__select__"] ?? 55,
          ellipsis: false,
          fixed: "left",
          resizable: false
        }
      ]
    : [];

  const colByKey = new Map(props.columns.map((c) => [c.key, c]));
  const userCols: RenderColumn[] = [...columnStates.value]
    .sort((a, b) => a.order - b.order)
    .filter((s) => s.visible)
    .map((s) => {
      const orig = colByKey.get(s.key);
      if (!orig) return null;
      const base = normalizeColumn(orig);
      // 用户配置的 fixed 覆盖原列的 fixed；仅开放"左固定"
      base.fixed = s.fixed ? "left" : undefined;
      return base;
    })
    .filter((c): c is RenderColumn => !!c);

  return [...selectableCol, ...userCols];
});

/** 所有列宽之和，作为 head / body / row 的显式宽度。 */
const totalColumnWidth = computed(() =>
  renderColumns.value.reduce((sum, c) => sum + c.width, 0)
);

/** 列 key → 最小宽度，拖拽时用。 */
const colMinWidths = computed<Record<string, number>>(() => {
  const map: Record<string, number> = {};
  for (const c of props.columns) map[c.key] = c.minWidth ?? 80;
  return map;
});

let resizingKey = "";
let startX = 0;
let startW = 0;
let resizeRaf = 0;

function onResizeDown(e: MouseEvent, key: string) {
  resizingKey = key;
  startX = e.clientX;
  startW = colWidths[key];

  const move = (ev: MouseEvent) => {
    if (!resizingKey) return;
    if (resizeRaf) return;
    resizeRaf = requestAnimationFrame(() => {
      resizeRaf = 0;
      const delta = ev.clientX - startX;
      const min = colMinWidths.value[resizingKey] ?? 80;
      colWidths[resizingKey] = Math.max(min, startW + delta);
    });
  };

  const up = () => {
    resizingKey = "";
    if (resizeRaf) {
      cancelAnimationFrame(resizeRaf);
      resizeRaf = 0;
    }
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
  };

  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
}

// ---- 固定列偏移：缓存成 computed，避免每个单元格重新计算。 ----
const leftOffsets = computed(() => {
  const map: Record<string, number> = {};
  let acc = 0;
  for (const c of renderColumns.value) {
    if (c.fixed === "left") {
      map[c.key] = acc;
      acc += c.width;
    }
  }
  return map;
});

/**
 * 单元格样式。sticky 的 scroll ancestor 必须是 `.vtable-scroll`（唯一滚动容器），
 * 这样 head 固定列和所有 body 行固定列对横滚的响应一致——不再出现"只有首行/表头
 * 固定列跟着滚，其他行不跟"的错位。
 */
function cellStyle(col: RenderColumn, isHead = false) {
  const style: Record<string, string | number> = {
    width: `${col.width}px`,
    minWidth: `${col.width}px`
  };
  if (col.fixed === "left") {
    style.position = "sticky";
    style.left = `${leftOffsets.value[col.key] || 0}px`;
    // head 固定列 > body 固定列 > 普通列（普通 head/body 走 class 指定）
    style.zIndex = isHead ? 4 : 2;
  }
  return style;
}

/** 取单元格显示值；formatter 只调用一次，ellipsis tooltip 复用同一结果。 */
function renderCell(row: any, col: RenderColumn, index: number): string {
  const raw = row?.[col.key];
  if (col.formatter) return String(col.formatter(row, raw, index) ?? "");
  return raw == null ? "" : String(raw);
}

async function copyCellText(text: string) {
  const value = text.trim();
  if (!props.copyable || !value) return;
  await copyText(value);
  ElMessage.success("已复制");
}

// ---- 虚拟滚动（单滚动容器版） ----

const scrollRef = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
/** 滚动容器的当前视口高度，随 ResizeObserver 更新。auto 模式下初始值是兜底。 */
const clientHeight = ref(props.height ?? 360);
let scrollRaf = 0;

/**
 * 滚动事件：只读 scrollTop，raf 节流。
 * 读 scrollRef 而不是 e.target，因为 Vue 在快速滚动时可能复用事件对象。
 */
function onScroll() {
  if (scrollRaf) return;
  scrollRaf = requestAnimationFrame(() => {
    scrollRaf = 0;
    const el = scrollRef.value;
    if (el) scrollTop.value = el.scrollTop;
  });
}

/** 表头占 HEADER_HEIGHT，剩余才是 body 可视高度（决定渲染多少行）。 */
const bodyViewHeight = computed(() => Math.max(0, clientHeight.value - HEADER_HEIGHT));

// ---- autoRowHeight：行高随内容自适应（用 ResizeObserver 测每行实际高度） ----

/**
 * rowKey → 已测得的行高（px，向上取整）。
 * 未在 map 里的行用 props.itemHeight 兜底——所以 autoRowHeight 模式下 itemHeight
 * 等于"未测量行的预估高度"，合理传一个接近典型行的值能让滚动条少抖几下。
 */
const measuredRowHeights = ref<Map<string, number>>(new Map());

let rowResizeObserver: ResizeObserver | null = null;
const observedRowEls = new WeakSet<Element>();
let pendingMeasurements: Map<string, number> | null = null;
let measureRaf = 0;

/**
 * 在 rAF 边界把 pending 的测量结果合并提交，触发 rowOffsets 重算。
 * 用 `next = new Map(cur)` 替换引用而不是就地改 Map，让 Vue 的依赖系统看见变化。
 */
function flushMeasurements() {
  measureRaf = 0;
  if (!pendingMeasurements) return;
  const cur = measuredRowHeights.value;
  let changed = false;
  for (const [k, v] of pendingMeasurements) {
    if (cur.get(k) !== v) {
      changed = true;
      break;
    }
  }
  if (changed) {
    const next = new Map(cur);
    for (const [k, v] of pendingMeasurements) next.set(k, v);
    measuredRowHeights.value = next;
  }
  pendingMeasurements = null;
}

/** 把单条测量塞进 pending 队列，等下一帧统一刷。 */
function scheduleMeasurementUpdate(key: string, height: number) {
  if (!pendingMeasurements) pendingMeasurements = new Map();
  pendingMeasurements.set(key, height);
  if (measureRaf) return;
  measureRaf = requestAnimationFrame(flushMeasurements);
}

/**
 * 行高的前缀和：offsets[i] = 第 i 行顶端的 y 坐标；offsets[count] = 总高度。
 * autoRowHeight 关时返回 null，直接走 `index * itemHeight` 的简单数学。
 */
const rowOffsets = computed<number[] | null>(() => {
  if (!props.autoRowHeight) return null;
  const rows = pagedRows.value;
  const map = measuredRowHeights.value;
  const fallback = props.itemHeight;
  const offsets: number[] = new Array(rows.length + 1);
  offsets[0] = 0;
  let acc = 0;
  for (let i = 0; i < rows.length; i++) {
    const key = String(getRowKey(rows[i], i));
    acc += map.get(key) ?? fallback;
    offsets[i + 1] = acc;
  }
  return offsets;
});

/** 二分查找：在 offsets 里返回最大的 i 使 offsets[i] <= top。复杂度 O(log n)。 */
function findRowAt(offsets: number[], top: number, count: number): number {
  if (count <= 0) return 0;
  if (top <= 0) return 0;
  if (top >= offsets[count]) return count - 1;
  let lo = 0;
  let hi = count - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (offsets[mid] <= top) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/**
 * 在 visibleRows 变化后用 nextTick 走一遍 .body 下的所有 `.row[data-row-key]`，
 * 把没观察过的元素加入观察列表。已观察元素通过 WeakSet 去重——元素被 Vue 卸载
 * 后会被 GC，WeakSet 自动清掉，无需手工 unobserve。
 */
function reconcileRowObservers() {
  if (!props.autoRowHeight || !rowResizeObserver) return;
  const body = scrollRef.value?.querySelector(".body");
  if (!body) return;
  const els = body.querySelectorAll<HTMLElement>(".row[data-row-key]");
  els.forEach((el) => {
    if (!observedRowEls.has(el)) {
      rowResizeObserver!.observe(el);
      observedRowEls.add(el);
    }
  });
}

// 行数据彻底变了（如扫描重启 / 任务切换）时清空 measured map，避免老 key 堆积让
// map 无界增长。阈值用 2× 是给 partial 增量保留缓冲——同一次扫描里行数是稳定的。
watch(
  () => props.rows.length,
  (v) => {
    if (props.autoRowHeight && measuredRowHeights.value.size > Math.max(64, v * 2)) {
      measuredRowHeights.value = new Map();
    }
  }
);

/** body 的占位总高度；虚拟滚动用这个撑起滚动条。autoRowHeight 模式下取 prefix sum 的最后一项。 */
const totalBodyHeight = computed(() => {
  if (props.autoRowHeight) {
    const offsets = rowOffsets.value;
    return offsets ? offsets[pagedRows.value.length] : 0;
  }
  return pagedRows.value.length * props.itemHeight;
});

/**
 * 当前内容是否真的需要纵向滚动。
 *
 * 浏览器 `overflow-y: auto` 自带"内容超过 clientHeight 才出滚动条"的判断，但
 * `clientHeight` 已经被横向滚动条挤掉了 ~17px——所以当横向滚动条出现（fit-width
 * 退化）时，原本"刚好装下"的内容会被挤压触发**不必要的纵向滚动条**：
 * - 容器实高 400px → clientHeight 无横滚时 400 / 有横滚时 383
 * - HEADER 36px + 内容 360px = 396px
 * - 无横滚：总高 396 ≤ 容器 400，不出纵向条 ✓
 * - 有横滚：总高 396 > clientHeight 383，浏览器误判为"需要纵向条"
 *
 * 这里加回横向滚动条占用的空间得到"假设没有横向滚动条时的最大可视高度"，
 * 用它判断；判定不需要纵向滚动时通过 `.vtable-scroll--no-vscroll` 强制
 * `overflow-y: hidden` 覆盖默认 auto。fit-width 生效（无横向条）时 allowance
 * 为 0，不影响"内容真的多到溢出"的常规纵向滚动。
 */
const verticallyOverflowing = computed(() => {
  const horizScrollAllowance = fitWidthApplied.value ? 0 : HORIZONTAL_SCROLLBAR_ALLOWANCE;
  return totalBodyHeight.value > bodyViewHeight.value + horizScrollAllowance;
});

/**
 * 当前该渲染的行区间。
 *
 * 固定行高分支：scrollTop 为 `.vtable-scroll` 的滚动偏移，`scrollTop = 0` 时表头
 * 刚好顶住视口，body 从视口 y=HEADER_HEIGHT 开始。第一可见行 index =
 * floor(scrollTop / itemHeight)。
 *
 * autoRowHeight 分支：用前缀和 + 二分找首行，然后线性扫到 bottom 找末行。
 */
const visibleRange = computed(() => {
  const count = pagedRows.value.length;
  if (count === 0) return { start: 0, end: 0 };
  const overscan = props.overscan;
  if (props.autoRowHeight) {
    const offsets = rowOffsets.value!;
    const top = scrollTop.value;
    const bottom = top + bodyViewHeight.value;
    const first = findRowAt(offsets, top, count);
    let last = first;
    while (last < count && offsets[last] < bottom) last++;
    return {
      start: Math.max(0, first - overscan),
      end: Math.min(count, last + overscan)
    };
  }
  const first = Math.floor(scrollTop.value / props.itemHeight);
  const visibleCount = Math.ceil(bodyViewHeight.value / props.itemHeight);
  const start = Math.max(0, first - overscan);
  const end = Math.min(count, first + visibleCount + overscan);
  return { start, end };
});

const visibleRows = computed(() => {
  const { start, end } = visibleRange.value;
  const offsets = props.autoRowHeight ? rowOffsets.value : null;
  const rows = pagedRows.value.slice(start, end);
  return rows.map((data, i) => {
    const index = start + i;
    return {
      data,
      index,
      top: offsets ? offsets[index] : index * props.itemHeight,
      key: String(getRowKey(data, index))
    };
  });
});

let resizeObserver: ResizeObserver | null = null;
/**
 * ResizeObserver 回调用 rAF 合并：浏览器在同一帧里可能连续触发多次（比如
 * 纵向滚动条出现 → 宽度变化 → 布局重绘 → 再触发），直接 set 会让 fit-width
 * 的列宽在同一帧里反复计算，视觉上就是"滚动条闪烁"。
 */
let roRaf = 0;
let pendingH = 0;
let pendingW = 0;
onMounted(() => {
  if (scrollRef.value) {
    clientHeight.value = scrollRef.value.clientHeight;
    containerWidth.value = scrollRef.value.clientWidth;
    resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        pendingH = entry.contentRect.height;
        pendingW = entry.contentRect.width;
      }
      if (roRaf) return;
      roRaf = requestAnimationFrame(() => {
        roRaf = 0;
        if (pendingH > 0 && pendingH !== clientHeight.value) clientHeight.value = pendingH;
        if (pendingW > 0 && pendingW !== containerWidth.value) containerWidth.value = pendingW;
      });
    });
    resizeObserver.observe(scrollRef.value);
  }

  // autoRowHeight 模式下创建行高观察器；关掉时完全不创建,零开销。
  if (props.autoRowHeight) {
    rowResizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const el = entry.target as HTMLElement;
        const key = el.dataset.rowKey;
        if (!key) continue;
        // contentRect.height 不含 border / 含 padding;.row 没设 border、padding 为 0,
        // 所以等同 offsetHeight。Math.ceil 避免 sub-pixel 累加成可见误差。
        const h = Math.ceil(entry.contentRect.height);
        if (h > 0) scheduleMeasurementUpdate(key, h);
      }
    });
    nextTick(reconcileRowObservers);
  }
});

// visibleRows 变了(滚动 / 数据更新)就把新出现的行接入 ResizeObserver。
// 用 nextTick 等 DOM 真渲染完;reconcile 内部用 WeakSet 去重,不会重复 observe。
watch(visibleRows, () => {
  if (!props.autoRowHeight) return;
  nextTick(reconcileRowObservers);
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  rowResizeObserver?.disconnect();
  if (scrollRaf) cancelAnimationFrame(scrollRaf);
  if (roRaf) cancelAnimationFrame(roRaf);
  if (measureRaf) cancelAnimationFrame(measureRaf);
});

// 数据大幅度变化（例如切 tab、刷新数据）时把滚动条归零，避免 scrollTop 停留
// 在旧数据的底部，让用户看到一段空白。
watch(
  () => props.rows.length,
  (v, old) => {
    if (!scrollRef.value) return;
    // 仅在行数变短且 scrollTop 超出新内容范围时回滚
    const maxTop = Math.max(0, v * props.itemHeight - bodyViewHeight.value);
    if (scrollRef.value.scrollTop > maxTop) {
      scrollRef.value.scrollTop = maxTop;
      scrollTop.value = maxTop;
    }
    // 行数从 0 变为非 0 时不归零；行数突变但不是减少不归零
    void old;
  }
);

function onPageChange(v: number) {
  internalPage.value = v;
  emit("pageChange", v);
}
function onPageSizeChange(v: number) {
  internalPageSize.value = v;
  internalPage.value = 1;
  emit("pageSizeChange", v);
}

// ---- 列设置面板 ----

/** Popover 展开状态。 */
const configVisible = ref(false);

/**
 * 列设置弹层里展示的条目：按当前 order 升序，包含原列定义的 label。
 * selection 列不出现在里面。
 */
const configItems = computed(() => {
  const colByKey = new Map(props.columns.map((c) => [c.key, c]));
  return [...columnStates.value]
    .sort((a, b) => a.order - b.order)
    .map((s) => ({
      key: s.key,
      label: colByKey.get(s.key)?.label ?? s.key,
      visible: s.visible,
      fixed: s.fixed,
      order: s.order
    }));
});

// ---- 列拖拽调序：走 pointer 事件（Tauri WebView 会吞掉 HTML5 DnD，显示"禁止"光标） ----

const dragFromOrder = ref<number | null>(null);
const dragOverOrder = ref<number | null>(null);
let dragPointerId = -1;

function onHandlePointerDown(idx: number, e: PointerEvent) {
  if (e.button !== 0) return;
  e.preventDefault();
  dragFromOrder.value = idx;
  dragOverOrder.value = idx;
  dragPointerId = e.pointerId;
  window.addEventListener("pointermove", onDragPointerMove);
  window.addEventListener("pointerup", onDragPointerEnd);
  window.addEventListener("pointercancel", onDragPointerEnd);
}

function onDragPointerMove(e: PointerEvent) {
  if (dragFromOrder.value == null) return;
  if (e.pointerId !== dragPointerId) return;
  const el = document.elementFromPoint(e.clientX, e.clientY);
  if (!el) return;
  const row = (el as Element).closest?.(".col-config-row") as HTMLElement | null;
  if (row && row.dataset.orderIdx != null) {
    const idx = Number(row.dataset.orderIdx);
    if (Number.isFinite(idx)) dragOverOrder.value = idx;
  }
}

function onDragPointerEnd(e: PointerEvent) {
  window.removeEventListener("pointermove", onDragPointerMove);
  window.removeEventListener("pointerup", onDragPointerEnd);
  window.removeEventListener("pointercancel", onDragPointerEnd);
  if (e.pointerId !== dragPointerId) return;
  const from = dragFromOrder.value;
  const to = dragOverOrder.value;
  dragFromOrder.value = null;
  dragOverOrder.value = null;
  dragPointerId = -1;
  if (from != null && to != null && from !== to) {
    moveColumn(from, to);
  }
}
</script>

<template>
  <div class="vtable" :class="{ 'vtable--auto-height': autoHeight }">
    <div v-if="columnConfigurable" class="vtable-toolbar">
      <el-popover
        v-model:visible="configVisible"
        placement="bottom-end"
        :width="320"
        trigger="click"
        :popper-style="{ padding: '8px' }"
      >
        <template #reference>
          <el-button size="small" text>
            <el-icon><Setting /></el-icon>
            <span style="margin-left:4px">列设置</span>
          </el-button>
        </template>

        <div class="col-config">
          <div class="col-config-header">
            <span>拖拽 ⋮⋮ 调整顺序 · 勾选显示/固定</span>
          </div>
          <div class="col-config-list">
            <div
              v-for="(item, idx) in configItems"
              :key="item.key"
              class="col-config-row"
              :data-order-idx="idx"
              :class="{
                'is-dragging': dragFromOrder === idx,
                'is-drop-target': dragOverOrder === idx && dragFromOrder !== idx
              }"
            >
              <span
                class="drag-handle"
                title="按住拖拽调整顺序"
                @pointerdown="(e) => onHandlePointerDown(idx, e)"
              >⋮⋮</span>
              <el-checkbox
                :model-value="item.visible"
                size="small"
                @update:model-value="(v: boolean) => setVisible(item.key, v)"
              >显示</el-checkbox>
              <el-checkbox
                :model-value="item.fixed"
                :disabled="!item.visible"
                size="small"
                @update:model-value="(v: boolean) => setFixed(item.key, v)"
              >固定</el-checkbox>
              <span class="col-label" :title="item.label">{{ item.label }}</span>
            </div>
          </div>
          <div class="col-config-footer">
            <el-button size="small" text @click="resetColumnConfig">重置默认</el-button>
          </div>
        </div>
      </el-popover>
    </div>

    <div
      ref="scrollRef"
      class="vtable-scroll"
      :class="{
        'vtable-scroll--fit': fitWidthApplied,
        'vtable-scroll--no-vscroll': !verticallyOverflowing
      }"
      :style="autoHeight ? undefined : { height: `${height}px` }"
      @scroll="onScroll"
    >
      <div
        class="vtable-content"
        :style="fitWidthApplied ? { width: '100%' } : { width: `${totalColumnWidth}px` }"
      >
        <div class="head">
          <div
            v-for="col in renderColumns"
            :key="col.key"
            class="cell head-cell"
            :class="{ 'fixed-left': col.fixed === 'left' }"
            :style="cellStyle(col, true)"
          >
            <template v-if="col.key === '__select__'">
              <el-checkbox
                :model-value="isAllSelected"
                :indeterminate="isIndeterminate"
                @update:model-value="(v: boolean) => toggleAll(v)"
              />
            </template>
            <template v-else>
              {{ col.label }}
            </template>
            <span
              v-if="col.resizable"
              class="resize-handle"
              @mousedown.stop.prevent="(e) => onResizeDown(e, col.key)"
            />
          </div>
        </div>

        <div
          class="body"
          :style="{
            height: pagedRows.length ? `${totalBodyHeight}px` : `${bodyViewHeight}px`
          }"
        >
          <template v-if="pagedRows.length > 0">
            <div
              v-for="row in visibleRows"
              :key="row.key"
              class="row"
              :data-row-key="row.key"
              :style="autoRowHeight
                ? { top: `${row.top}px`, minHeight: `${itemHeight}px` }
                : { top: `${row.top}px`, height: `${itemHeight}px` }"
              @click="emit('rowClick', row.data)"
            >
              <template v-for="col in renderColumns" :key="col.key">
                <div
                  class="cell"
                  :class="{ ellipsis: col.ellipsis, 'fixed-left': col.fixed === 'left' }"
                  :style="cellStyle(col)"
                  @dblclick.stop="col.key !== '__select__' && copyCellText(renderCell(row.data, col, row.index))"
                >
                  <template v-if="col.key === '__select__'">
                    <el-checkbox
                      :model-value="selectedSet.has(getRowKey(row.data, row.index))"
                      @update:model-value="(v: boolean) => toggleRow(row.data, row.index, v)"
                      @click.stop
                    />
                  </template>

                  <template v-else-if="col.slotName">
                    <slot
                      :name="col.slotName"
                      :row="row.data"
                      :value="row.data[col.key]"
                      :index="row.index"
                    />
                  </template>

                  <template v-else>
                    <el-tooltip
                      v-if="col.ellipsis"
                      :content="renderCell(row.data, col, row.index)"
                      placement="top"
                      :show-after="300"
                      :hide-after="0"
                    >
                      <span class="ellipsis-text">{{ renderCell(row.data, col, row.index) }}</span>
                    </el-tooltip>
                    <template v-else>
                      {{ renderCell(row.data, col, row.index) }}
                    </template>
                  </template>
                </div>
              </template>
            </div>
          </template>

          <div v-else class="empty-state">
            <el-empty :description="emptyText" :image-size="60" />
          </div>
        </div>
      </div>
    </div>

    <div v-if="showPagination" class="pagination">
      <el-pagination
        :current-page="internalPage"
        :page-size="internalPageSize"
        :total="totalRows"
        :page-sizes="props.pagination?.pageSizes || [10, 20, 50, 100]"
        background
        layout="total, prev, pager, next, sizes"
        @update:current-page="onPageChange"
        @update:page-size="onPageSizeChange"
      />
    </div>
  </div>
</template>

<style scoped>
.vtable {
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-md);
  background: var(--ff-bg-panel);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  width: 100%;
  min-width: 0;
}

/* auto-height 模式：父必须是 flex column；表格自己 flex:1 撑满，内部滚动容器
   再 flex:1 占据除 toolbar 外的剩余空间，避免外层再算一个"表格高度"魔数。 */
.vtable--auto-height {
  flex: 1;
  min-height: 0;
}
.vtable--auto-height .vtable-scroll {
  flex: 1;
  min-height: 0;
}
.vtable-toolbar {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  padding: 2px 8px;
  min-height: 32px;
  border-bottom: 1px solid var(--ff-border-subtle);
  background: var(--ff-bg-header);
}

/* 单一滚动容器：横向 / 纵向都走它，所有 sticky 的 scroll ancestor 就是它。
 *
 * `scrollbar-gutter: stable` 是修掉"滚动条反复闪烁"的关键：永远预留 ~15px
 * 纵向滚动条宽度，clientWidth 不再随内容行多寡震荡，fit-width 模式下列宽
 * 计算一次到位；不然纵滚出现 → 宽度缩 → 列宽缩 → 纵滚消失 → 宽度复位 →
 * 列宽复位 → 纵滚再出现…无限循环。 */
.vtable-scroll {
  overflow-y: auto;
  overflow-x: auto;
  scrollbar-gutter: stable;
}
/* fit-width 仅在"列总最小宽度 ≤ 容器宽度"时实际生效（见 `fitWidthApplied`），
   生效时 fittedColWidths 把列总宽锚定为容器宽度，横滚理论上永远不该出现，强制
   `overflow-x: hidden` 兜底 1px 舍入误差，避免横滚条偶尔闪一下。

   反过来当容器太窄、所有列的 minWidth 之和已经撑出容器外时，`fitWidthApplied`
   退化为 false，本类不应用，外层 `overflow-x: auto` 接管显示横滚条 —— 此时用户
   缩窄窗口能看到横向滚动条来浏览被挤出去的列，而不是被 `overflow-x: hidden`
   一刀切。 */
.vtable-scroll--fit {
  overflow-x: hidden;
}

/* 横向滚动条出现时（fit-width 退化），它占据底部 ~17px 让 clientHeight 缩水，
   原本"刚好装下"的内容会被这 17px 挤压触发不必要的纵向滚动条。`verticallyOverflowing`
   computed 在 JS 侧加回这部分高度做判断，认定实际不需要纵向滚动时强制
   `overflow-y: hidden` 覆盖默认 auto。

   仅在"被横向滚动条挤压才超出"的边缘情况起作用：内容真的多到超过容器高度时
   `verticallyOverflowing = true`，本类不应用，纵向滚动条照常出。 */
.vtable-scroll--no-vscroll {
  overflow-y: hidden;
}

/* 内容层：宽度锚定 totalColumnWidth；min-width: 100% 让 totalColumnWidth 小于
 * 容器时撑满容器，这样表头/行不再只占一小段、右边留一片容器底色的空白 */
.vtable-content {
  position: relative;
  min-width: 100%;
}

/* 表头：纵向粘在滚动容器顶部；横向撑满内容层，最后一列右侧留白用表头底色填充 */
.head {
  display: flex;
  width: 100%;
  position: sticky;
  top: 0;
  z-index: 3;
  background: var(--ff-bg-header);
  border-bottom: 1px solid var(--ff-border-subtle);
}

.head-cell {
  height: 36px;
  font-weight: 600;
  background: var(--ff-bg-header);
  color: var(--ff-text-secondary);
  font-size: var(--ff-font-sm);
}

/* body 作为 abs rows 的 containing block，行按 top 绝对定位 */
.body {
  position: relative;
  width: 100%;
}

.row {
  position: absolute;
  left: 0;
  width: 100%;
  display: flex;
  border-bottom: 1px solid var(--ff-border-subtle);
  background: var(--ff-bg-panel);
}

/* hover 时行和行内固定列一起翻色，避免 sticky 的不透明底色盖住 hover 效果 */
.row:hover,
.row:hover .cell.fixed-left {
  background: var(--ff-bg-panel-hover);
}

.cell {
  padding: 0 10px;
  display: flex;
  align-items: center;
  font-size: var(--ff-font-sm);
  color: var(--ff-text-primary);
  border-right: 1px solid var(--ff-border-subtle);
  box-sizing: border-box;
  position: relative; /* 给 resize-handle 作为定位参考 */
  user-select: text;
}
.cell:last-child {
  border-right: 0;
}

/* 固定列要有不透明底色，否则横向滚动时后面的文字会透上来 */
.cell.fixed-left {
  background: var(--ff-bg-panel);
}
.head-cell.fixed-left {
  background: var(--ff-bg-header);
}

.ellipsis {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.ellipsis-text {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  display: block;
  min-width: 0;
  user-select: text;
}

.resize-handle {
  position: absolute;
  right: -3px;
  top: 0;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  user-select: none;
  z-index: 5;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  min-height: 120px;
}

.pagination {
  padding: 8px;
  display: flex;
  justify-content: flex-end;
  border-top: 1px solid var(--ff-border-subtle);
}

/* ---- 列设置面板 ---- */
.col-config {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.col-config-header {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-muted);
  padding: 2px 4px;
}
.col-config-list {
  display: flex;
  flex-direction: column;
  max-height: 360px;
  overflow-y: auto;
  border-top: 1px solid var(--ff-border-subtle);
  border-bottom: 1px solid var(--ff-border-subtle);
}
.col-config-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 4px;
  user-select: none;
}
.col-config-row:hover {
  background: var(--ff-bg-muted);
}
.col-config-row.is-dragging {
  opacity: 0.5;
}
.col-config-row.is-drop-target {
  box-shadow: inset 0 2px 0 0 var(--ff-accent);
}
.drag-handle {
  color: var(--ff-text-muted);
  font-weight: bold;
  cursor: grab;
  width: 16px;
  line-height: 1;
  letter-spacing: -2px;
  touch-action: none;
}
.drag-handle:active {
  cursor: grabbing;
}
.col-label {
  flex: 1;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: var(--ff-font-sm);
}
.col-config-footer {
  display: flex;
  justify-content: flex-end;
  padding: 0 4px;
}
</style>
