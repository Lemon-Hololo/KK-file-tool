<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { useElementSize, useStorage } from "@vueuse/core";

import { useModToolsStore } from "../stores/modTools";
import { exportModScanResult } from "../services/modTools";
import { EXTREME_SUFFIX_ROW_THRESHOLD, EXTREME_OVERSCAN, NORMAL_OVERSCAN } from "../constants/task";
import type { VirtualColumn } from "../types/virtualTable";
import VirtualTable from "./common/VirtualTable.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const store = useModToolsStore();

const panelRef = ref<HTMLElement | null>(null);
const { height: panelHeight } = useElementSize(panelRef);

const keyword = useStorage<string>("modScanKeyword", "Koikatsu");

const tableHeight = computed(() => {
  const h = panelHeight.value;
  if (!h) return 420;
  return Math.max(260, h - 180);
});

const tableData = computed<any[]>(() => store.scan.matches as any[]);
const isExtreme = computed(() => tableData.value.length > EXTREME_SUFFIX_ROW_THRESHOLD);

const columns = computed<VirtualColumn[]>(() => [
  { key: "filePath", label: "文件", minWidth: 360, ellipsis: true, resizable: true },
  { key: "author", label: "作者", width: 140, ellipsis: true, resizable: true },
  { key: "guid", label: "GUID", minWidth: 200, ellipsis: true, resizable: true },
  { key: "version", label: "版本", width: 120, resizable: true },
  { key: "matchedKeyword", label: "命中关键字", width: 140, resizable: true }
]);

async function startScan() {
  if (!keyword.value.trim()) {
    ElMessage.warning("请输入关键字");
    return;
  }
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;

  try {
    await store.startScan(normalized, keyword.value.trim());
    ElMessage.success("扫描已开始");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function stopScan() {
  try {
    await store.stopScan();
    ElMessage.info("已请求停止");
  } catch (e) {
    ElMessage.error(String(e));
  }
}

async function exportResult() {
  if (!store.scan.matches.length) {
    ElMessage.warning("没有可导出的结果");
    return;
  }
  const selected = await save({
    title: "导出扫描结果",
    defaultPath: `mod_scan_${store.scan.keyword || "result"}.txt`,
    filters: [{ name: "Text", extensions: ["txt"] }]
  });
  if (!selected) return;

  const lines = store.scan.matches.map((m) => m.filePath);
  await exportModScanResult(selected, lines);
  ElMessage.success(`已导出 ${lines.length} 条到 ${selected}`);
}

onMounted(() => {
  store.initScanEvents();
});

onBeforeUnmount(() => {
  // 保留 unlisten 交由 store 管理；切换 Tab 不强制取消
});
</script>

<template>
  <div ref="panelRef" class="mod-scan-panel">
    <el-card shadow="hover" class="main-card">
      <el-form inline class="top-form">
        <el-form-item label="关键字">
          <el-input v-model="keyword" placeholder="manifest.xml 中 <game> 标签内容，如 Koikatsu" style="width: 220px" />
        </el-form-item>
        <el-form-item>
          <el-space wrap>
            <el-button type="primary" :disabled="store.scan.running" @click="startScan">开始扫描</el-button>
            <el-button type="warning" :disabled="!store.scan.running" @click="stopScan">停止</el-button>
            <el-button :disabled="!store.scan.matches.length" @click="exportResult">导出 TXT</el-button>
          </el-space>
        </el-form-item>
      </el-form>

      <el-alert
        type="info"
        :closable="false"
        style="margin-bottom:8px"
        :title="`状态：${
          store.scan.running
            ? '扫描中...'
            : store.scan.taskId
              ? `已完成（匹配 ${store.scan.matches.length}，扫描 ${store.scan.totalScanned}，错误 ${store.scan.totalErrors}${store.scan.cancelled ? '，已取消' : ''}）`
              : '未开始'
        }`"
      />

      <el-alert
        v-if="isExtreme"
        type="warning"
        :closable="false"
        style="margin-bottom:8px"
        title="匹配结果较多，已启用极限性能模式"
      />

      <VirtualTable
        :rows="tableData"
        :columns="columns"
        :height="tableHeight"
        :item-height="36"
        :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
        row-key="filePath"
      />
    </el-card>
  </div>
</template>

<style scoped>
.mod-scan-panel {
  height: 100%;
  min-height: 0;
}
.main-card {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
</style>
