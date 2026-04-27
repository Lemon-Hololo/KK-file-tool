<script setup lang="ts">
/**
 * 任务控制按钮组：开始 / 暂停 / 继续 / 停止 + 当前状态标。
 *
 * 扫描阶段（stage === "scan"）只允许停止，暂停/继续禁用：扫描阶段没有
 * "安全中断点"，硬暂停会留下不完整的中间结果。
 */
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

const isScanStage = computed(() => props.stage === "scan");

const canStart = computed(
  () =>
    props.status === "Idle" ||
    props.status === "Completed" ||
    props.status === "Cancelled" ||
    props.status === "Failed"
);
const canPause = computed(() => props.status === "Running" && !isScanStage.value);
const canResume = computed(() => props.status === "Paused" && !isScanStage.value);
const canStop = computed(() => props.status === "Running" || props.status === "Paused");

const statusTone = computed(() => {
  switch (props.status) {
    case "Running":
      return "is-running";
    case "Paused":
      return "is-paused";
    case "Completed":
      return "is-success";
    case "Failed":
      return "is-danger";
    case "Cancelled":
      return "is-muted";
    default:
      return "is-idle";
  }
});

const statusLabel = computed(() => {
  const map: Record<string, string> = {
    Idle: "就绪",
    Running: "运行中",
    Paused: "已暂停",
    Completed: "已完成",
    Failed: "已失败",
    Cancelled: "已取消"
  };
  return map[props.status] ?? props.status;
});
</script>

<template>
  <div class="task-control">
    <el-button type="primary" size="small" :disabled="!canStart" @click="emit('start')">开始</el-button>
    <el-button size="small" :disabled="!canPause" @click="emit('pause')">暂停</el-button>
    <el-button size="small" :disabled="!canResume" @click="emit('resume')">继续</el-button>
    <el-button type="danger" plain size="small" :disabled="!canStop" @click="emit('stop')">停止</el-button>
    <span class="status-chip" :class="statusTone">
      <span class="status-dot" />
      {{ statusLabel }}
    </span>
  </div>
</template>

<style scoped>
.task-control {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.status-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2px 10px;
  font-size: var(--ff-font-xs);
  font-weight: 600;
  border-radius: 999px;
  background: var(--ff-bg-muted);
  color: var(--ff-text-secondary);
  margin-left: 4px;
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ff-text-muted);
}
.is-idle .status-dot { background: var(--ff-text-muted); }
.is-running {
  background: var(--ff-accent-soft);
  color: var(--ff-accent);
}
.is-running .status-dot {
  background: var(--ff-accent);
  animation: ff-pulse 1.2s ease-in-out infinite;
}
.is-paused {
  background: var(--ff-warning-soft);
  color: var(--ff-warning);
}
.is-paused .status-dot { background: var(--ff-warning); }
.is-success {
  background: var(--ff-success-soft);
  color: var(--ff-success);
}
.is-success .status-dot { background: var(--ff-success); }
.is-danger {
  background: var(--ff-danger-soft);
  color: var(--ff-danger);
}
.is-danger .status-dot { background: var(--ff-danger); }

@keyframes ff-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(1.35); }
}
</style>
