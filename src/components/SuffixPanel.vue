<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useElementSize, useStorage } from "@vueuse/core";

import { useSuffixStore } from "../stores/suffix";
import {
  EXTREME_SUFFIX_ROW_THRESHOLD,
  EXTREME_OVERSCAN,
  NORMAL_OVERSCAN
} from "../constants/task";
import type { VirtualColumn } from "../types/virtualTable";
import VirtualTable from "./common/VirtualTable.vue";

const props = defineProps<{
  paths: string[];
  ensureNormalizedPaths: () => Promise<string[] | null>;
}>();

const suffixStore = useSuffixStore();

const panelRef = ref<HTMLElement | null>(null);
const { height: panelHeight } = useElementSize(panelRef);

const targetSuffix = useStorage<string>("suffixTargetSuffix", "txt");
const suffixRecordName = useStorage<string>("suffixRecordName", "");

const selectedSuffixOldPaths = ref<string[]>([]);

const tableHeight = computed(() => {
  const h = panelHeight.value;
  if (!h) return 420;
  return Math.max(260, h - 180);
});

const tableData = computed<any[]>(() => (suffixStore.lastApplyResult?.items || suffixStore.previewList) as any[]);
const isExtreme = computed(() => tableData.value.length > EXTREME_SUFFIX_ROW_THRESHOLD);

const columns = computed<VirtualColumn[]>(() => [
  { key: "oldPath", label: "修改前", minWidth: 320, ellipsis: true, resizable: true },
  { key: "newPath", label: "修改后", minWidth: 320, ellipsis: true, resizable: true },
  { key: "status", label: "状态", width: 100, resizable: true },
  { key: "message", label: "信息", minWidth: 180, ellipsis: true, resizable: true }
]);

function onSelectionChange(rows: any[]) {
  selectedSuffixOldPaths.value = rows.map((x) => x.oldPath).filter(Boolean);
}

async function previewSuffix() {
  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;

  await suffixStore.preview(normalized, targetSuffix.value);
  selectedSuffixOldPaths.value = [];
  ElMessage.success(`预览完成，共 ${suffixStore.previewList.length} 条`);
}

async function applySuffix() {
  if (!suffixStore.previewList.length) {
    ElMessage.warning("请先执行预览");
    return;
  }

  await ElMessageBox.confirm("确认执行后缀批量修改？", "确认", { type: "warning" });

  const normalized = await props.ensureNormalizedPaths();
  if (!normalized) return;

  const selectedOldPaths =
    selectedSuffixOldPaths.value.length > 0
      ? selectedSuffixOldPaths.value
      : suffixStore.previewList.map((x) => x.oldPath);

  const result = await suffixStore.apply(
    normalized,
    targetSuffix.value,
    suffixRecordName.value || null,
    selectedOldPaths
  );

  ElMessage.success(`完成：成功 ${result.success}，失败 ${result.failed}`);
  await suffixStore.refreshRecords();
  selectedSuffixOldPaths.value = [];
}

async function rollbackLastApply() {
  const recordId = suffixStore.lastApplyResult?.recordId;
  if (!recordId) {
    ElMessage.warning("没有可撤回记录");
    return;
  }

  const check = await suffixStore.checkRollback(recordId);
  if (check.missingPaths.length) {
    await ElMessageBox.confirm(
      `有 ${check.missingPaths.length} 个文件不存在，仅撤回存在文件，继续？`,
      "缺失提示",
      { type: "warning" }
    );
  }

  const resp = await suffixStore.rollback(recordId, null, true);
  ElMessage.success(`撤回完成：成功 ${resp.success}，失败 ${resp.failed}，跳过缺失 ${resp.skippedMissing}`);
}

async function rollbackSelectedRows() {
  const recordId = suffixStore.lastApplyResult?.recordId;
  if (!recordId) {
    ElMessage.warning("没有可撤回记录");
    return;
  }

  const mapByOldPath = new Map((suffixStore.lastApplyResult?.items || []).map((x) => [x.oldPath, x.itemId]));
  const itemIds = selectedSuffixOldPaths.value
    .map((p) => mapByOldPath.get(p))
    .filter((x): x is number => !!x);

  if (!itemIds.length) {
    ElMessage.warning("请先勾选要撤回的记录项");
    return;
  }

  const check = await suffixStore.checkRollback(recordId, itemIds);
  if (check.missingPaths.length) {
    await ElMessageBox.confirm(
      `选中项中有 ${check.missingPaths.length} 个文件不存在，仅撤回存在文件，继续？`,
      "缺失提示",
      { type: "warning" }
    );
  }

  const resp = await suffixStore.rollback(recordId, itemIds, true);
  ElMessage.success(`部分撤回完成：成功 ${resp.success}，失败 ${resp.failed}，跳过缺失 ${resp.skippedMissing}`);
}
</script>

<template>
  <div ref="panelRef" class="suffix-panel">
    <el-card shadow="hover" class="main-card">
      <el-form inline class="top-form">
        <el-form-item label="目标后缀">
          <el-input v-model="targetSuffix" placeholder="如 txt 或 .txt" />
        </el-form-item>
        <el-form-item label="记录名">
          <el-input v-model="suffixRecordName" placeholder="可空，默认时间命名" />
        </el-form-item>
        <el-form-item>
          <el-space wrap>
            <el-button @click="previewSuffix">预览</el-button>
            <el-button type="primary" @click="applySuffix">确认修改</el-button>
            <el-button type="warning" @click="rollbackLastApply">撤回本次</el-button>
            <el-button @click="rollbackSelectedRows">撤回选中</el-button>
          </el-space>
        </el-form-item>
      </el-form>

      <el-alert
        type="info"
        :closable="false"
        style="margin-bottom:8px"
        :title="`当前勾选：${selectedSuffixOldPaths.length}（未勾选时默认处理全部预览项）`"
      />

      <el-alert
        v-if="isExtreme"
        type="warning"
        :closable="false"
        style="margin-bottom:8px"
        title="数据量较大，已启用极限性能模式（低 overscan + 虚拟渲染）"
      />

      <VirtualTable
        :rows="tableData"
        :columns="columns"
        :height="tableHeight"
        :item-height="36"
        :overscan="isExtreme ? EXTREME_OVERSCAN : NORMAL_OVERSCAN"
        row-key="oldPath"
        selectable
        @selectionChange="onSelectionChange"
      />
    </el-card>
  </div>
</template>

<style scoped>
.suffix-panel {
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
