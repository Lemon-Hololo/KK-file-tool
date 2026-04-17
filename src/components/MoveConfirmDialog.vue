<script setup lang="ts">
import { computed } from "vue";
import type { MoveSummary } from "../types/moveReport";
import { formatBytes } from "../utils/format";

const props = defineProps<{
  modelValue: boolean;
  summary: MoveSummary | null;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: boolean): void;
  (e: "confirm"): void;
}>();

const visible = computed({
  get: () => props.modelValue,
  set: (v) => emit("update:modelValue", v)
});
</script>

<template>
  <el-dialog v-model="visible" title="移动确认" width="520px">
    <template v-if="summary">
      <p><b>目标目录：</b>{{ summary.targetDir }}</p>
      <p><b>待移动文件数：</b>{{ summary.totalSelected }}</p>
      <p><b>预计释放空间：</b>{{ formatBytes(summary.totalSize) }}</p>
      <el-alert title="确认后将执行移动，并记录操作报告" type="warning" :closable="false" />
    </template>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" @click="emit('confirm')">确认移动</el-button>
    </template>
  </el-dialog>
</template>