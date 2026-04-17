<script setup lang="ts">
import type { MoveReport } from "../types/moveReport";
import { formatBytes } from "../utils/format";

defineProps<{
  modelValue: boolean;
  report: MoveReport | null;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
}>();
</script>

<template>
  <el-dialog :model-value="modelValue" title="移动操作报告" width="760px"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)">
    <template v-if="report">
      <el-descriptions :column="2" border>
        <el-descriptions-item label="报告ID">{{ report.reportId }}</el-descriptions-item>
        <el-descriptions-item label="任务ID">{{ report.taskId }}</el-descriptions-item>
        <el-descriptions-item label="目标目录">{{ report.targetDir }}</el-descriptions-item>
        <el-descriptions-item label="释放空间">{{ formatBytes(report.releasedBytes) }}</el-descriptions-item>
        <el-descriptions-item label="成功">{{ report.totalSuccess }}</el-descriptions-item>
        <el-descriptions-item label="失败">{{ report.totalFailed }}</el-descriptions-item>
      </el-descriptions>

      <el-divider>失败明细</el-divider>
      <el-table :data="report.failedItems || []" border size="small" max-height="300">
        <el-table-column prop="srcPath" label="源路径" min-width="260" />
        <el-table-column prop="errorCode" label="错误码" width="120" />
        <el-table-column prop="errorMessage" label="错误信息" min-width="220" />
      </el-table>
    </template>
  </el-dialog>
</template>