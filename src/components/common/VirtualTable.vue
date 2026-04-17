<script setup lang="ts">
import { computed, ref, watch, reactive } from "vue";
import { useVirtualList } from "@vueuse/core";
import type { PaginationConfig, RenderColumn, VirtualColumn } from "../../types/virtualTable";

const props = withDefaults(
  defineProps<{
    rows: any[];
    columns: VirtualColumn[];
    height?: number;
    itemHeight?: number;
    overscan?: number;
    rowKey?: string | ((row: any) => string | number);
    selectable?: boolean;
    pagination?: PaginationConfig;
  }>(),
  {
    height: 360,
    itemHeight: 36,
    overscan: 10,
    rowKey: "id",
    selectable: false
  }
);

const emit = defineEmits<{
  (e: "rowClick", row: any): void;
  (e: "selectionChange", rows: any[]): void;
  (e: "pageChange", page: number): void;
  (e: "pageSizeChange", pageSize: number): void;
}>();

const selectedSet = ref<Set<string | number>>(new Set());

function getRowKey(row: any) {
  if (typeof props.rowKey === "function") return props.rowKey(row);
  return row?.[props.rowKey] ?? row?.oldPath ?? row?.absPath ?? Math.random();
}

watch(
  () => props.rows,
  () => {
    const keys = new Set(props.rows.map((r) => getRowKey(r)));
    selectedSet.value = new Set(Array.from(selectedSet.value).filter((k) => keys.has(k)));
    emitSelection();
  },
  { deep: true }
);

// 全选 / 取消全选
const isAllSelected = computed(() => {
  const rows = pagedRows.value;
  return rows.length > 0 && rows.every((r) => selectedSet.value.has(getRowKey(r)));
});

const isIndeterminate = computed(() => {
  const rows = pagedRows.value;
  const count = rows.filter((r) => selectedSet.value.has(getRowKey(r))).length;
  return count > 0 && count < rows.length;
});

function toggleAll(checked: boolean) {
  if (checked) {
    for (const r of pagedRows.value) {
      selectedSet.value.add(getRowKey(r));
    }
  } else {
    for (const r of pagedRows.value) {
      selectedSet.value.delete(getRowKey(r));
    }
  }
  emitSelection();
}

function toggleRow(row: any, checked: boolean) {
  const key = getRowKey(row);
  if (checked) selectedSet.value.add(key);
  else selectedSet.value.delete(key);
  emitSelection();
}

function emitSelection() {
  const rowMap = new Map(props.rows.map((r) => [getRowKey(r), r]));
  const rows = Array.from(selectedSet.value)
    .map((k) => rowMap.get(k))
    .filter(Boolean);
  emit("selectionChange", rows);
}

// pagination
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

// width + resize
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

function normalizeColumn(c: VirtualColumn): RenderColumn {
  return {
    key: c.key,
    label: c.label,
    width: colWidths[c.key] || c.width || c.minWidth || 120,
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
          width: 55,
          ellipsis: false,
          fixed: "left",
          resizable: false
        }
      ]
    : [];

  return [...selectableCol, ...props.columns.map(normalizeColumn)];
});

let resizingKey = "";
let startX = 0;
let startW = 0;
let _resizeRaf = 0;

function onResizeDown(e: MouseEvent, key: string) {
  resizingKey = key;
  startX = e.clientX;
  startW = colWidths[key];

  const move = (ev: MouseEvent) => {
    if (!resizingKey) return;
    if (_resizeRaf) return;
    _resizeRaf = requestAnimationFrame(() => {
      _resizeRaf = 0;
      const delta = ev.clientX - startX;
      colWidths[resizingKey] = Math.max(80, startW + delta);
    });
  };

  const up = () => {
    resizingKey = "";
    if (_resizeRaf) {
      cancelAnimationFrame(_resizeRaf);
      _resizeRaf = 0;
    }
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
  };

  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
}

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

const rightOffsets = computed(() => {
  const map: Record<string, number> = {};
  let acc = 0;
  const rev = [...renderColumns.value].reverse();
  for (const c of rev) {
    if (c.fixed === "right") {
      map[c.key] = acc;
      acc += c.width;
    }
  }
  return map;
});

function cellStyle(col: RenderColumn) {
  const style: Record<string, string> = {
    width: `${col.width}px`,
    minWidth: `${col.width}px`
  };
  if (col.fixed === "left") {
    style.position = "sticky";
    style.left = `${leftOffsets.value[col.key] || 0}px`;
    style.zIndex = "3";
    style.background = "var(--el-bg-color)";
  }
  if (col.fixed === "right") {
    style.position = "sticky";
    style.right = `${rightOffsets.value[col.key] || 0}px`;
    style.zIndex = "3";
    style.background = "var(--el-bg-color)";
  }
  return style;
}

// virtual
const { list, containerProps, wrapperProps } = useVirtualList(pagedRows, {
  itemHeight: props.itemHeight,
  overscan: props.overscan
});

function onPageChange(v: number) {
  internalPage.value = v;
  emit("pageChange", v);
}
function onPageSizeChange(v: number) {
  internalPageSize.value = v;
  internalPage.value = 1;
  emit("pageSizeChange", v);
}
</script>

<template>
  <div class="vtable">
    <div class="head">
      <div
        v-for="col in renderColumns"
        :key="col.key"
        class="cell head-cell"
        :style="cellStyle(col)"
      >
        <template v-if="col.key === '__select__'">
          <el-checkbox
            :model-value="isAllSelected"
            :indeterminate="isIndeterminate"
            @update:model-value="(v:boolean)=>toggleAll(v)"
          />
        </template>
        <template v-else>
          {{ col.label }}
        </template>
        <span
          v-if="col.resizable"
          class="resize-handle"
          @mousedown.stop.prevent="(e)=>onResizeDown(e, col.key)"
        />
      </div>
    </div>

    <div v-bind="containerProps" class="body" :style="{ height: `${height}px` }">
      <div v-bind="wrapperProps">
        <div
          v-for="item in list"
          :key="getRowKey(item.data)"
          class="row"
          @click="emit('rowClick', item.data)"
        >
          <template v-for="col in renderColumns" :key="col.key">
            <div class="cell" :class="{ ellipsis: col.ellipsis }" :style="cellStyle(col)">
              <template v-if="col.key === '__select__'">
                <el-checkbox
                  :model-value="selectedSet.has(getRowKey(item.data))"
                  @update:model-value="(v:boolean)=>toggleRow(item.data, v)"
                  @click.stop
                />
              </template>

              <template v-else-if="col.slotName">
                <slot :name="col.slotName" :row="item.data" :value="item.data[col.key]" :index="item.index" />
              </template>

              <template v-else>
                <el-tooltip
                  v-if="col.ellipsis"
                  :content="String(col.formatter ? col.formatter(item.data, item.data[col.key], item.index) : item.data[col.key] ?? '')"
                  placement="top"
                  :show-after="300"
                  :hide-after="0"
                >
                  <span class="ellipsis-text">{{
                    col.formatter
                      ? col.formatter(item.data, item.data[col.key], item.index)
                      : item.data[col.key]
                  }}</span>
                </el-tooltip>
                <template v-else>
                  {{
                    col.formatter
                      ? col.formatter(item.data, item.data[col.key], item.index)
                      : item.data[col.key]
                  }}
                </template>
              </template>
            </div>
          </template>
        </div>
      </div>
    </div>

    <div v-if="showPagination" class="pagination">
      <el-pagination
        :current-page="internalPage"
        :page-size="internalPageSize"
        :total="totalRows"
        :page-sizes="props.pagination?.pageSizes || [10,20,50,100]"
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
  border: 1px solid var(--el-border-color-light);
  border-radius: 8px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.head {
  display: flex;
  border-bottom: 1px solid var(--el-border-color-light);
  background: var(--el-fill-color-light);
  z-index: 5;
}
.body {
  overflow: auto;
}
.row {
  display: flex;
  height: 36px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.row:hover {
  background: var(--el-fill-color-lighter);
}
.cell {
  padding: 0 8px;
  display: flex;
  align-items: center;
  font-size: 12px;
  border-right: 1px solid var(--el-border-color-lighter);
  box-sizing: border-box;
}
.head-cell {
  position: relative;
  height: 36px;
  font-weight: 600;
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
}
.resize-handle {
  position: absolute;
  right: -3px;
  top: 0;
  width: 6px;
  height: 100%;
  cursor: col-resize;
}
.pagination {
  padding: 8px;
  display: flex;
  justify-content: flex-end;
  border-top: 1px solid var(--el-border-color-light);
}
</style>
