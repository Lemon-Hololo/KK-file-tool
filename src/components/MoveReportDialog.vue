<script setup lang="ts">
/** 移动操作报告对话框。失败明细用虚拟表展示，保持与其他面板一致。 */

import type { MoveReport } from "../types/moveReport";
import type { VirtualColumn } from "../types/virtualTable";
import { formatBytes } from "../utils/format";
import VirtualTable from "./common/VirtualTable.vue";

defineProps<{
  modelValue: boolean;
  report: MoveReport | null;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
}>();

const failedColumns: VirtualColumn[] = [
  { key: "srcPath", label: "源路径", minWidth: 260, ellipsis: true, resizable: true },
  { key: "errorCode", label: "错误码", width: 120 },
  { key: "errorMessage", label: "错误信息", minWidth: 220, ellipsis: true, resizable: true }
];
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="移动操作报告"
    width="760px"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
  >
    <template v-if="report">
      <el-descriptions :column="2" border size="small">
        <el-descriptions-item label="报告 ID">{{ report.reportId }}</el-descriptions-item>
        <el-descriptions-item label="任务 ID">{{ report.taskId }}</el-descriptions-item>
        <el-descriptions-item label="目标目录" :span="2">{{ report.targetDir }}</el-descriptions-item>
        <el-descriptions-item label="释放空间">{{ formatBytes(report.releasedBytes) }}</el-descriptions-item>
        <el-descriptions-item label="成功 / 失败">
          <span style="color: var(--ff-success); font-weight: 600;">{{ report.totalSuccess }}</span>
          <span style="margin: 0 4px; color: var(--ff-text-muted)">/</span>
          <span style="color: var(--ff-danger); font-weight: 600;">{{ report.totalFailed }}</span>
        </el-descriptions-item>
      </el-descriptions>

      <div class="failed-header">失败明细</div>
      <VirtualTable
        :rows="report.failedItems || []"
        :columns="failedColumns"
        :height="280"
        :item-height="36"
        row-key="srcPath"
        fit-width
        empty-text="没有失败条目"
      />
    </template>
  </el-dialog>
</template>

<style scoped>
.failed-header {
  margin: var(--ff-space-3) 0 var(--ff-space-2);
  font-size: var(--ff-font-sm);
  font-weight: 600;
  color: var(--ff-text-secondary);
}
</style>
