<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { useConfigStore } from "../stores/config";
import { THEME_OPTIONS } from "../constants/theme";

const configStore = useConfigStore();
const customDbPath = ref("");

onMounted(async () => {
  await Promise.all([
    configStore.loadDbInfo(),
    configStore.loadCpuCount(),
  ]);
  customDbPath.value = configStore.dbPathInfo?.customPath ?? "";
});

async function save() {
  await configStore.saveSettings();
  ElMessage.success("配置已保存");
}

async function changeTheme(v: any) {
  await configStore.changeTheme(v);
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
      "此操作将永久删除数据库中的所有数据，包括：\n- 所有哈希索引记录\n- 所有移动报告\n- 所有后缀修改记录\n- 所有配置设置\n\n删除后将重建空数据库，此操作不可恢复！",
      "确认删除数据库",
      {
        confirmButtonText: "确认删除",
        cancelButtonText: "取消",
        type: "error",
        confirmButtonClass: "el-button--danger",
      }
    );
  } catch {
    return; // 用户取消
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
</script>

<template>
  <el-row :gutter="16">
    <el-col :span="15">
      <el-card>
        <template #header>配置中心</template>
        <el-form label-width="220px">
          <el-form-item label="默认保留策略">
            <el-radio-group v-model="configStore.settings.keepPolicy">
              <el-radio value="newest">保留最新</el-radio>
              <el-radio value="oldest">保留最旧</el-radio>
            </el-radio-group>
          </el-form-item>

          <el-form-item label="移动目标目录">
            <el-input v-model="configStore.settings.moveTargetPath" placeholder="为空使用程序目录/temp_moved_files" />
          </el-form-item>

          <el-form-item label="保存哈希索引记录">
            <el-switch v-model="configStore.settings.saveRecordEnabled" />
          </el-form-item>

          <el-form-item label="使用上一次记录">
            <el-switch v-model="configStore.settings.useLastRecordEnabled" />
          </el-form-item>

          <el-form-item label="包含当前目录重复">
            <el-switch v-model="configStore.settings.includeCurrentFolderDuplicates" />
          </el-form-item>

          <el-form-item label="处理核心数">
            <div style="width: 100%;">
              <el-slider
                v-model="configStore.settings.threadCount"
                :min="0"
                :max="configStore.cpuCount || 16"
                :step="1"
                :format-tooltip="threadCountLabel"
                show-stops
              />
              <div style="font-size: 12px; opacity: 0.6; margin-top: -4px;">
                当前：{{ configStore.settings.threadCount === 0 ? `自动（全部 ${configStore.cpuCount} 核）` : `${configStore.settings.threadCount} 核` }}
                ，可用 CPU 核心数：{{ configStore.cpuCount }}
              </div>
            </div>
          </el-form-item>

          <el-form-item label="主题模式">
            <el-segmented
              :options="THEME_OPTIONS as any"
              v-model="configStore.settings.themeMode"
              @change="changeTheme"
            />
          </el-form-item>

          <el-form-item>
            <el-button type="primary" @click="save">保存配置</el-button>
          </el-form-item>
        </el-form>
      </el-card>

      <!-- 数据库管理 -->
      <el-card style="margin-top: 16px;">
        <template #header>数据库管理</template>
        <el-form label-width="220px">
          <el-form-item label="当前数据库路径">
            <el-input :model-value="configStore.dbPathInfo?.currentPath ?? ''" disabled />
          </el-form-item>

          <el-form-item label="默认路径">
            <el-input :model-value="configStore.dbPathInfo?.defaultPath ?? ''" disabled />
          </el-form-item>

          <el-form-item label="自定义数据库路径">
            <div style="display: flex; gap: 8px; width: 100%;">
              <el-input v-model="customDbPath" placeholder="留空使用默认路径" clearable />
              <el-button type="primary" @click="saveDbPath">保存</el-button>
              <el-button @click="clearDbPath">恢复默认</el-button>
            </div>
            <div style="font-size: 12px; opacity: 0.6; margin-top: 4px;">
              修改后需重启应用生效。建议使用本地磁盘路径。
            </div>
          </el-form-item>

          <el-form-item label="危险操作">
            <el-button type="danger" @click="handleDeleteDb">删除数据库</el-button>
            <span style="font-size: 12px; opacity: 0.6; margin-left: 8px;">
              删除所有数据并重建空数据库
            </span>
          </el-form-item>
        </el-form>
      </el-card>
    </el-col>

    <el-col :span="9">
      <el-card>
        <template #header>说明</template>
        <ul>
          <li>保存记录：会保存哈希索引到 SQLite。</li>
          <li>移动成功：会从索引中移除对应条目。</li>
          <li>重名冲突：自动采用 name (1).ext 方式处理。</li>
          <li>处理核心数：设为 0 则自动使用全部 CPU 核心。数值越大处理越快，但会占用更多系统资源。</li>
          <li>自定义数据库路径：可将数据库存放到其他位置，修改后需重启应用。</li>
          <li>删除数据库：将永久删除所有记录数据，操作不可恢复。</li>
        </ul>
      </el-card>
    </el-col>
  </el-row>
</template>
