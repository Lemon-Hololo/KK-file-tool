<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  status: string;
  stage?: string;
}>();

const emit = defineEmits<{
  (e: "start"): void;
  (e: "pause"): void;
  (e: "resume"): void;
  (e: "stop"): void;
}>();

// 扫描阶段只允许停止，禁用暂停/继续
const isScanStage = computed(() => props.stage === "scan");

// 按钮启用状态
const canStart = computed(() => props.status === "Idle" || props.status === "Completed" || props.status === "Cancelled" || props.status === "Failed");
const canPause = computed(() => props.status === "Running" && !isScanStage.value);
const canResume = computed(() => props.status === "Paused" && !isScanStage.value);
const canStop = computed(() => props.status === "Running" || props.status === "Paused");
</script>

<template>
  <el-space wrap>
    <el-button type="primary" :disabled="!canStart" @click="emit('start')">开始</el-button>
    <el-button :disabled="!canPause" @click="emit('pause')">暂停</el-button>
    <el-button :disabled="!canResume" @click="emit('resume')">继续</el-button>
    <el-button type="danger" :disabled="!canStop" @click="emit('stop')">停止</el-button>
    <el-tag>{{ status }}</el-tag>
  </el-space>
</template>
