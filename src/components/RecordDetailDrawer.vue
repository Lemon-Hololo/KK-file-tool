<!-- src/components/RecordDetailDrawer.vue -->
<script setup lang="ts">
import { computed, ref } from "vue";
import type { HashIndexRecord } from "../types/record";
import type { VirtualColumn } from "../types/virtualTable";
import { formatBytes } from "../utils/format";
import VirtualTable from "./common/VirtualTable.vue";
import { stripWindowsExtendedPrefix } from "../utils/path";

const props = defineProps<{
  modelValue: boolean;
  record: HashIndexRecord | null;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
}>();

const keyword = ref("");

// 过滤后的行数据，直接传给 VirtualTable
const filteredRows = computed(() => {
  const entries = props.record?.entries ?? [];
  const kw = keyword.value.trim().toLowerCase();
  if (!kw) return entries;
  return entries.filter((x) =>
    `${x.hash} ${x.filePath}`.toLowerCase().includes(kw)
  );
});

// 列定义
const columns: VirtualColumn[] = [
  {
    key: "hash",
    label: "Hash",
    width: 280,
    minWidth: 180,
    ellipsis: true,
    resizable: true,
  },
  {
    key: "filePath",
    label: "路径",
    width: 380,
    minWidth: 200,
    ellipsis: true,
    resizable: true,
    formatter: (_row: any, val: string) =>
      stripWindowsExtendedPrefix(val ?? ""),
  },
  {
    key: "fileSize",
    label: "大小",
    width: 110,
    resizable: false,
    formatter: (_row: any, val: number) => formatBytes(val ?? 0),
  },
  {
    key: "status",
    label: "状态",
    width: 90,
    resizable: false,
  },
];
</script>

<template>
  <el-drawer :model-value="modelValue" size="65%" title="记录详情"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)">
    <template v-if="record">
      <!-- 基础信息 -->
      <el-descriptions :column="1" border size="small">
        <el-descriptions-item label="记录ID">{{ record.recordId }}</el-descriptions-item>
        <el-descriptions-item label="记录名">{{ record.recordName }}</el-descriptions-item>
        <el-descriptions-item label="创建时间">{{ record.createdAt }}</el-descriptions-item>
        <el-descriptions-item label="路径">
          {{ record.sourcePaths.join(" ; ") }}
        </el-descriptions-item>
      </el-descriptions>

      <!-- 搜索栏 + 条数提示 -->
      <div style="display:flex;align-items:center;gap:12px;margin:12px 0;">
        <el-input v-model="keyword" clearable placeholder="搜索 Hash / 路径" style="flex:1;">
          <template #prefix>
            <el-icon>
              <Search />
            </el-icon>
          </template>
        </el-input>
        <span style="font-size:12px;opacity:0.55;white-space:nowrap;">
          {{ filteredRows.length }} / {{ record.entries?.length ?? 0 }} 条
        </span>
      </div>

      <!-- 虚拟表：高度设为抽屉内剩余空间（描述+搜索约140px，留底部余量） -->
      <VirtualTable :rows="filteredRows" :columns="columns" :height="480" :item-height="36" :overscan="12"
        row-key="hash" />
    </template>

    <!-- record 为空时的占位 -->
    <el-empty v-else description="暂无数据" />
  </el-drawer>
</template>
