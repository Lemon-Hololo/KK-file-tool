<script setup lang="ts">
/**
 * 配置中心页。
 *
 * 布局：单一滚动容器 `.settings-scroll`（overflow-y: auto）承载表单卡片组。
 * 用自有 Panel 按主题分组——基础 / 性能 / 预览 / 工具默认值 / 数据库管理。
 */
import { onMounted, ref } from "vue";
import { watchDebounced } from "@vueuse/core";
import { ElMessage, ElMessageBox } from "element-plus";
import { useConfigStore } from "../stores/config";
import { THEME_OPTIONS } from "../constants/theme";
import Panel from "../components/common/Panel.vue";

const configStore = useConfigStore();
const customDbPath = ref("");
const autoSaveState = ref<"saved" | "saving" | "error">("saved");

onMounted(async () => {
  await Promise.all([configStore.loadDbInfo(), configStore.loadCpuCount()]);
  customDbPath.value = configStore.dbPathInfo?.customPath ?? "";
});

async function changeTheme(v: any) {
  configStore.applyThemeMode(v);
}

async function saveDbPath() {
  try {
    await configStore.setCustomDbPath(customDbPath.value);
    ElMessage.success("数据库路径已保存，重启应用后生效");
  } catch (e: any) {
    ElMessage.error(e?.toString() ?? "保存失败");
  }
}

async function clearDbPath() {
  customDbPath.value = "";
  await configStore.setCustomDbPath("");
  ElMessage.success("已恢复默认数据库路径，重启应用后生效");
}

async function handleDeleteDb() {
  try {
    await ElMessageBox.confirm(
      "此操作将永久删除数据库中的所有数据（哈希索引 / 移动报告 / 后缀修改 / 配置设置），删除后将重建空数据库，此操作不可恢复！",
      "确认删除数据库",
      {
        confirmButtonText: "确认删除",
        cancelButtonText: "取消",
        type: "error",
        confirmButtonClass: "el-button--danger"
      }
    );
  } catch {
    return;
  }

  try {
    await configStore.deleteDb();
    ElMessage.success("数据库已删除并重建");
  } catch (e: any) {
    ElMessage.error(e?.toString() ?? "删除失败");
  }
}

function threadCountLabel(val: number) {
  return val === 0 ? "自动" : `${val} 核`;
}

function ioMultiplierLabel(val: number) {
  return `×${val}`;
}

watchDebounced(
  () => JSON.stringify(configStore.settings),
  async () => {
    autoSaveState.value = "saving";
    try {
      await configStore.saveSettings();
      autoSaveState.value = "saved";
    } catch (e: any) {
      autoSaveState.value = "error";
      ElMessage.error(e?.toString() ?? "自动保存失败");
    }
  },
  { debounce: 400, maxWait: 1500 }
);
</script>

<template>
  <div class="settings-page ff-scroll">
    <div class="settings-inner">
      <!-- 基础配置 -->
      <Panel title="基础" :padded="false">
        <div class="form">
          <div class="form-row">
            <label class="label">默认保留策略</label>
            <el-radio-group v-model="configStore.settings.keepPolicy">
              <el-radio value="newest">保留最新</el-radio>
              <el-radio value="oldest">保留最旧</el-radio>
            </el-radio-group>
          </div>

          <div class="form-row">
            <label class="label">移动目标目录</label>
            <el-input
              v-model="configStore.settings.moveTargetPath"
              placeholder="为空使用程序目录/temp_moved_files"
              class="flex-input"
            />
          </div>

          <div class="form-row">
            <label class="label">保存哈希索引记录</label>
            <el-switch v-model="configStore.settings.saveRecordEnabled" />
          </div>

          <div class="form-row">
            <label class="label">使用上一次记录</label>
            <el-switch v-model="configStore.settings.useLastRecordEnabled" />
          </div>

          <div class="form-row">
            <label class="label">包含当前目录重复</label>
            <el-switch v-model="configStore.settings.includeCurrentFolderDuplicates" />
          </div>

          <div class="form-row">
            <label class="label">主题模式</label>
            <el-segmented
              :options="THEME_OPTIONS as any"
              v-model="configStore.settings.themeMode"
              @change="changeTheme"
            />
          </div>
        </div>
      </Panel>

      <!-- 性能 -->
      <Panel title="性能" :padded="false">
        <div class="form">
          <div class="form-row">
            <label class="label">处理核心数</label>
            <div class="flex-input">
              <el-slider
                v-model="configStore.settings.threadCount"
                :min="0"
                :max="configStore.cpuCount || 16"
                :step="1"
                :format-tooltip="threadCountLabel"
                show-stops
              />
              <div class="hint">
                当前：{{ configStore.settings.threadCount === 0 ? `自动（全部 ${configStore.cpuCount} 核）` : `${configStore.settings.threadCount} 核` }} · 可用 {{ configStore.cpuCount }} 核
              </div>
            </div>
          </div>

          <div class="form-row">
            <label class="label">IO 并发倍率</label>
            <div class="flex-input">
              <el-slider
                v-model="configStore.settings.ioConcurrencyMultiplier"
                :min="1"
                :max="16"
                :step="1"
                :format-tooltip="ioMultiplierLabel"
                show-stops
              />
              <div class="hint">
                实际 IO 并发 = 有效线程数 × 本倍率。SSD/NVMe 可上调到 4~8，HDD 建议降到 1
              </div>
            </div>
          </div>

          <div class="form-row">
            <label class="label">日志保留上限</label>
            <div class="flex-input">
              <el-input-number
                v-model="configStore.settings.logMaxLength"
                :min="500"
                :max="100000"
                :step="500"
                controls-position="right"
              />
              <span class="hint-inline">条；低配机调小省内存，长跑任务调大看全历史</span>
            </div>
          </div>

          <div class="form-row">
            <label class="label">极限模式行数阈值</label>
            <div class="flex-input">
              <el-input-number
                v-model="configStore.settings.extremeRowThreshold"
                :min="1000"
                :max="1000000"
                :step="1000"
                controls-position="right"
              />
              <span class="hint-inline">行；虚拟表超过此行数会降级 overscan 与分段渲染</span>
            </div>
          </div>
        </div>
      </Panel>

      <!-- 预览 -->
      <Panel title="预览" :padded="false">
        <div class="form">
          <div class="form-row">
            <label class="label">文本预览最大</label>
            <div class="flex-input">
              <el-input-number
                v-model="configStore.settings.textPreviewMaxKb"
                :min="16"
                :max="10240"
                :step="16"
                controls-position="right"
              />
              <span class="hint-inline">KiB；超过部分会被截断</span>
            </div>
          </div>

          <div class="form-row">
            <label class="label">压缩包预览条目上限</label>
            <div class="flex-input">
              <el-input-number
                v-model="configStore.settings.zipPreviewMaxEntries"
                :min="100"
                :max="100000"
                :step="100"
                controls-position="right"
              />
              <span class="hint-inline">条；超过部分不展示</span>
            </div>
          </div>
        </div>
      </Panel>

      <!-- 工具默认值 -->
      <Panel title="工具默认值" :padded="false">
        <div class="form">
          <div class="form-row">
            <label class="label">Mod 扫描默认关键字</label>
            <el-input
              v-model="configStore.settings.modScanDefaultKeyword"
              placeholder="manifest.xml 中 <game> 标签内容，如 Koikatsu"
              class="flex-input"
            />
          </div>

          <div class="form-row">
            <label class="label">默认后缀目标</label>
            <div class="flex-input">
              <el-input
                v-model="configStore.settings.suffixDefaultTarget"
                placeholder="如 txt 或 .txt"
                style="max-width: 220px"
              />
              <span class="hint-inline">仅对首次打开面板生效；之后以面板本地保存为准</span>
            </div>
          </div>
        </div>
      </Panel>

      <!-- 数据库管理 -->
      <Panel title="数据库管理" :padded="false">
        <div class="form">
          <div class="form-row">
            <label class="label">当前数据库路径</label>
            <el-input :model-value="configStore.dbPathInfo?.currentPath ?? ''" disabled class="flex-input" />
          </div>

          <div class="form-row">
            <label class="label">默认路径</label>
            <el-input :model-value="configStore.dbPathInfo?.defaultPath ?? ''" disabled class="flex-input" />
          </div>

          <div class="form-row">
            <label class="label">自定义数据库路径</label>
            <div class="flex-input db-path-group">
              <el-input v-model="customDbPath" placeholder="留空使用默认路径" clearable />
              <el-button type="primary" @click="saveDbPath">保存</el-button>
              <el-button @click="clearDbPath">恢复默认</el-button>
            </div>
          </div>

          <div class="form-row">
            <label class="label">危险操作</label>
            <div class="flex-input">
              <el-button type="danger" @click="handleDeleteDb">删除数据库</el-button>
              <span class="hint-inline">删除所有数据并重建空数据库，不可恢复</span>
            </div>
          </div>
        </div>
      </Panel>

      <!-- 操作条 -->
      <div class="footer-bar">
        <span class="footer-hint">修改后自动保存</span>
        <span v-if="autoSaveState === 'saving'" class="footer-state">正在保存…</span>
        <span v-else-if="autoSaveState === 'saved'" class="footer-state">已保存</span>
        <span v-else class="footer-state is-error">保存失败</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  min-height: 0;
}
.settings-inner {
  max-width: 960px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-4);
  padding-bottom: var(--ff-space-6);
}

.form {
  display: flex;
  flex-direction: column;
}

.form-row {
  display: grid;
  grid-template-columns: 180px 1fr;
  gap: var(--ff-space-4);
  align-items: center;
  padding: var(--ff-space-3) var(--ff-space-4);
  border-bottom: 1px solid var(--ff-border-subtle);
  min-height: 52px;
}
.form-row:last-child {
  border-bottom: 0;
}

.label {
  font-size: var(--ff-font-sm);
  font-weight: 500;
  color: var(--ff-text-secondary);
}

.flex-input {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.db-path-group {
  flex-direction: row;
  align-items: center;
  gap: 8px;
}
.db-path-group .el-input {
  flex: 1;
}

.hint {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
}
.hint-inline {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
  margin-left: 10px;
}

.footer-bar {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 12px;
  padding-top: var(--ff-space-2);
}

.footer-hint,
.footer-state {
  font-size: var(--ff-font-sm);
  color: var(--ff-text-muted);
}

.footer-state.is-error {
  color: var(--ff-danger);
}

@media (max-width: 700px) {
  .form-row {
    grid-template-columns: 1fr;
    gap: var(--ff-space-1);
  }
}
</style>
