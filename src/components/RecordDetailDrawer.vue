<script setup lang="ts">
/**
 * 详情抽屉：哈希记录详情。搜索 + 虚拟表 auto-height，flex 链一路透传。
 */
import { computed, ref } from "vue";
import { Search } from "@element-plus/icons-vue";
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

const filteredRows = computed(() => {
  const entries = props.record?.entries ?? [];
  const kw = keyword.value.trim().toLowerCase();
  if (!kw) return entries;
  return entries.filter((x) => `${x.hash} ${x.filePath}`.toLowerCase().includes(kw));
});

const columns: VirtualColumn[] = [
  {
    key: "hash",
    label: "Hash",
    width: 280,
    minWidth: 180,
    ellipsis: true,
    resizable: true
  },
  {
    key: "filePath",
    label: "路径",
    width: 380,
    minWidth: 200,
    ellipsis: true,
    resizable: true,
    formatter: (_row: any, val: string) => stripWindowsExtendedPrefix(val ?? "")
  },
  {
    key: "fileSize",
    label: "大小",
    width: 110,
    resizable: false,
    formatter: (_row: any, val: number) => formatBytes(val ?? 0)
  },
  {
    key: "status",
    label: "状态",
    width: 90,
    resizable: false
  }
];
</script>

<template>
  <el-drawer
    :model-value="modelValue"
    size="65%"
    title="哈希记录详情"
    class="detail-drawer"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
  >
    <template v-if="record">
      <div class="detail-body">
        <el-descriptions :column="1" border size="small">
          <el-descriptions-item label="记录 ID">{{ record.recordId }}</el-descriptions-item>
          <el-descriptions-item label="记录名">{{ record.recordName }}</el-descriptions-item>
          <el-descriptions-item label="创建时间">{{ record.createdAt }}</el-descriptions-item>
          <el-descriptions-item label="路径">
            {{ record.sourcePaths.join(" ; ") }}
          </el-descriptions-item>
        </el-descriptions>

        <div class="search-row">
          <el-input v-model="keyword" clearable placeholder="搜索 Hash / 路径" style="flex:1;">
            <template #prefix>
              <el-icon><Search /></el-icon>
            </template>
          </el-input>
          <span class="count-hint">
            {{ filteredRows.length }} / {{ record.entries?.length ?? 0 }} 条
          </span>
        </div>

        <VirtualTable
          :rows="filteredRows"
          :columns="columns"
          :item-height="36"
          :overscan="12"
          row-key="hash"
          column-config-key="records:hash-detail"
          fit-width
          class="detail-table"
        />
      </div>
    </template>

    <el-empty v-else description="暂无数据" />
  </el-drawer>
</template>

<style scoped>
.detail-drawer :deep(.el-drawer__body) {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}
.detail-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-3);
}
.search-row {
  display: flex;
  align-items: center;
  gap: var(--ff-space-3);
}
.count-hint {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-muted);
  white-space: nowrap;
}
.detail-table {
  flex: 1;
  min-height: 0;
}
</style>
