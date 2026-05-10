<script setup lang="ts">
/**
 * 配置中心页。
 *
 * 布局：左侧导航 + 右侧滚动内容。点击导航定位到对应分组；滚动时
 * 自动高亮当前可视分组（IntersectionObserver）。
 *
 * 滚动容器是 `.settings-scroll`（右侧），导航 `.settings-nav` 是它外层
 * 的网格列，所以 sticky 不会跟着内容滚走。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { watchDebounced } from "@vueuse/core";
import { ElMessage, ElMessageBox } from "element-plus";
import { open, save as saveFile } from "@tauri-apps/plugin-dialog";
import { Delete, Download, Edit, Folder, Plus, Upload } from "@element-plus/icons-vue";
import { useConfigStore } from "../stores/config";
import { THEME_OPTIONS } from "../constants/theme";
import { stripWindowsExtendedPrefix } from "../utils/path";
import { readTextFile, writeTextFile } from "../services/settings";
import { pickFolder } from "../composables/useFolderPicker";
import Panel from "../components/common/Panel.vue";

const configStore = useConfigStore();
const customDbPath = ref("");
const autoSaveState = ref<"saved" | "saving" | "error">("saved");

interface SectionItem {
  id: string;
  label: string;
}

const sections: SectionItem[] = [
  { id: "basic", label: "基础" },
  { id: "performance", label: "性能" },
  { id: "preview", label: "预览" },
  { id: "tools", label: "工具默认值" },
  { id: "pixiv", label: "Pixiv 标签" },
  { id: "database", label: "数据库管理" }
];

const activeSection = ref<string>(sections[0].id);
const scrollRef = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

onMounted(async () => {
  await Promise.all([configStore.loadDbInfo(), configStore.loadCpuCount()]);
  customDbPath.value = configStore.dbPathInfo?.customPath ?? "";
  setupSectionObserver();
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
});

/**
 * 用 IntersectionObserver 监听各 Panel 的可视状态，刷新左侧导航高亮。
 *
 * `rootMargin` 把判定区域上移 40%、下移 60%，使"当前 section"贴近用户视线
 * 中部而不是顶部边缘，否则在 section 之间过渡时高亮会闪。
 */
function setupSectionObserver() {
  const root = scrollRef.value;
  if (!root) return;
  observer?.disconnect();
  observer = new IntersectionObserver(
    (entries) => {
      const visible = entries
        .filter((e) => e.isIntersecting)
        .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
      if (visible.length) {
        const id = (visible[0].target as HTMLElement).dataset.section;
        if (id) activeSection.value = id;
      }
    },
    { root, threshold: [0, 0.25, 0.5], rootMargin: "-40% 0px -60% 0px" }
  );
  for (const s of sections) {
    const el = root.querySelector<HTMLElement>(`[data-section="${s.id}"]`);
    if (el) observer.observe(el);
  }
}

function scrollToSection(id: string) {
  const root = scrollRef.value;
  if (!root) return;
  const el = root.querySelector<HTMLElement>(`[data-section="${id}"]`);
  if (!el) return;
  activeSection.value = id;
  el.scrollIntoView({ behavior: "smooth", block: "start" });
}

async function changeTheme(v: any) {
  configStore.applyThemeMode(v);
}

/**
 * 弹出系统目录选择框，把选中的目录写到自定义数据库路径输入。
 *
 * 后端 `external_config::resolve_db_path` 已经做了"是目录就追加 kk-file-tool.db"的兜底，
 * 所以前端只把目录路径塞进去就行，不需要手动拼文件名。
 */
async function pickDbFolder() {
  const selected = await pickFolder("选择数据库存储目录");
  if (selected) customDbPath.value = selected;
}

/**
 * 弹出系统目录选择框，把选中的目录写到移动目标目录设置。
 *
 * 留空 → 后端去重移动会兜底到 `<exe_dir>/temp_moved_files`。
 */
async function pickMoveTargetFolder() {
  const selected = await pickFolder("选择移动目标目录");
  if (selected) configStore.settings.moveTargetPath = selected;
}

/**
 * 弹出系统目录选择框，把选中的目录写到 Mod 备份目录设置。
 *
 * 留空 → 后端 `services::mod_tools::backup::resolve_backup_root` 会兜底到
 * `<exe_dir>/mod-backups`。每条记录会自动落入 `<root>/<record_id>/`。
 */
async function pickModBackupFolder() {
  const selected = await pickFolder("选择 Mod 备份目录");
  if (selected) configStore.settings.modBackupDir = selected;
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

// ---------- 排除 tag：气泡输入 ----------
//
// 用 el-tag chip 列 + 一个原生 input 模拟"输入 tag 名,以 ; 分隔多个,自动变成气泡"。
// 不用 el-select multiple 的原因:
// 1) el-select 用回车提交,没法做"输入 ; 自动拆分",需要用户每次回车一次,体验差;
// 2) 半角 ;、全角 ;、IME 候选 ; 三种分隔符都要支持,el-select 没暴露能拦截输入的 hook;
// 3) 气泡样式自己控更稳,不会被 EP 升级折腾。
const excludedTagBuffer = ref("");
const excludedInputRef = ref<HTMLInputElement | null>(null);
const excludedSeparators = /[;；]/;
const localTranslationKey = ref("");
const localTranslationValue = ref("");
const localTranslationSearch = ref("");

function getExcludedTags(): string[] {
  return configStore.settings.pixivExcludedTags ?? [];
}

function normalizeTagList(tags: string[]) {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const t of tags) {
    const s = t.trim();
    if (!s || seen.has(s)) continue;
    seen.add(s);
    out.push(s);
  }
  return out;
}

/** 设置时去重 + trim,保证 chip 列里没有重复或空白项。 */
function setExcludedTags(tags: string[]) {
  configStore.settings.pixivExcludedTags = normalizeTagList(tags);
}

function removeExcludedTag(tag: string) {
  setExcludedTags(getExcludedTags().filter((t) => t !== tag));
}

/**
 * 输入框 input 事件:实时检查是否含有 ; / ；。
 * 含分隔符时,把分隔符前的所有部分作为 chip 提交,最后一段(可能在打字)留在 buffer 里。
 * 这样用户输入 "tag1;tag2;tag3" 就会得到三个 chip,buffer 清空;
 * 输入 "tag1;tag2" 后停笔,会得到 chip "tag1",buffer 留 "tag2"——再回车 / 失焦才提交最后这个。
 */
function onExcludedInput(e: Event) {
  const value = (e.target as HTMLInputElement).value;
  excludedTagBuffer.value = value;
  if (!excludedSeparators.test(value)) return;
  const parts = value.split(excludedSeparators);
  const toAdd = parts.slice(0, -1);
  const remaining = parts[parts.length - 1] ?? "";
  setExcludedTags([...getExcludedTags(), ...toAdd]);
  excludedTagBuffer.value = remaining;
}

/** 回车 / 失焦时把 buffer 整体提交。仍然走 ; 拆分,处理"用户在最后多打一个 ;"的情况。 */
function commitExcludedInput() {
  const value = excludedTagBuffer.value;
  if (!value.trim()) {
    excludedTagBuffer.value = "";
    return;
  }
  const parts = value.split(excludedSeparators).map((s) => s.trim()).filter(Boolean);
  if (parts.length === 0) {
    excludedTagBuffer.value = "";
    return;
  }
  setExcludedTags([...getExcludedTags(), ...parts]);
  excludedTagBuffer.value = "";
}

/** Backspace 在空 buffer 上时,删掉最后一个 chip(模仿 el-select multiple 的行为)。 */
function onExcludedKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    commitExcludedInput();
    return;
  }
  if (e.key === "Backspace" && !excludedTagBuffer.value) {
    const tags = getExcludedTags();
    if (tags.length > 0) {
      e.preventDefault();
      setExcludedTags(tags.slice(0, -1));
    }
  }
}

function getLocalTranslations(): Record<string, string> {
  return configStore.settings.pixivLocalTagTranslations ?? {};
}

function normalizeLocalTranslations(input: Record<string, unknown>) {
  const out: Record<string, string> = {};
  for (const [rawKey, rawValue] of Object.entries(input)) {
    const key = rawKey.trim();
    const value = typeof rawValue === "string" ? rawValue.trim() : String(rawValue ?? "").trim();
    if (!key || !value) continue;
    out[key] = value;
  }
  return out;
}

function setLocalTranslations(input: Record<string, unknown>) {
  configStore.settings.pixivLocalTagTranslations = normalizeLocalTranslations(input);
}

const localTranslationEntries = computed(() => {
  const q = localTranslationSearch.value.trim().toLowerCase();
  const entries = Object.entries(getLocalTranslations()).sort(([a], [b]) => a.localeCompare(b));
  if (!q) return entries;
  return entries.filter(([key, value]) =>
    key.toLowerCase().includes(q) || value.toLowerCase().includes(q)
  );
});

function upsertLocalTranslation() {
  const key = localTranslationKey.value.trim();
  const value = localTranslationValue.value.trim();
  if (!key || !value) {
    ElMessage.warning("请填写原 tag 和本地译名");
    return;
  }
  setLocalTranslations({ ...getLocalTranslations(), [key]: value });
  localTranslationKey.value = "";
  localTranslationValue.value = "";
}

function editLocalTranslation(key: string, value: string) {
  localTranslationKey.value = key;
  localTranslationValue.value = value;
}

function removeLocalTranslation(key: string) {
  const next = { ...getLocalTranslations() };
  delete next[key];
  setLocalTranslations(next);
}

function parseExcludedTagImport(content: string): string[] {
  const trimmed = content.trim();
  if (!trimmed) return [];
  try {
    const data = JSON.parse(trimmed) as unknown;
    if (Array.isArray(data)) return normalizeTagList(data.map((v) => String(v)));
    if (data && typeof data === "object" && Array.isArray((data as { tags?: unknown }).tags)) {
      return normalizeTagList(((data as { tags: unknown[] }).tags).map((v) => String(v)));
    }
  } catch {
    // 非 JSON 时继续按纯文本解析。
  }
  return normalizeTagList(trimmed.split(/[;\n\r,，；]+/));
}

function parseLocalTranslationImport(content: string): Record<string, string> {
  const trimmed = content.trim();
  if (!trimmed) return {};
  try {
    const data = JSON.parse(trimmed) as unknown;
    if (data && typeof data === "object" && !Array.isArray(data)) {
      const obj = data as Record<string, unknown>;
      if (obj.translations && typeof obj.translations === "object" && !Array.isArray(obj.translations)) {
        return normalizeLocalTranslations(obj.translations as Record<string, unknown>);
      }
      return normalizeLocalTranslations(obj);
    }
    if (Array.isArray(data)) {
      const out: Record<string, unknown> = {};
      for (const item of data) {
        if (Array.isArray(item) && item.length >= 2) {
          out[String(item[0])] = String(item[1]);
        } else if (item && typeof item === "object") {
          const row = item as Record<string, unknown>;
          const key = row.tag ?? row.original ?? row.key;
          const value = row.translation ?? row.value ?? row.display;
          if (key != null && value != null) out[String(key)] = String(value);
        }
      }
      return normalizeLocalTranslations(out);
    }
  } catch {
    // 非 JSON 时继续按一行一条解析。
  }

  const out: Record<string, unknown> = {};
  for (const line of trimmed.split(/\r?\n/)) {
    const s = line.trim();
    if (!s || s.startsWith("#")) continue;
    const match = s.match(/^(.+?)(?:\t|=|,|，)(.+)$/);
    if (!match) continue;
    out[match[1]] = match[2];
  }
  return normalizeLocalTranslations(out);
}

async function exportExcludedTags() {
  commitExcludedInput();
  try {
    const path = await saveFile({
      title: "导出排除 tag",
      defaultPath: "pixiv-excluded-tags.json",
      filters: [{ name: "JSON", extensions: ["json"] }]
    });
    if (!path) return;
    const payload = {
      type: "pixivExcludedTags",
      version: 1,
      tags: getExcludedTags()
    };
    await writeTextFile(path, `${JSON.stringify(payload, null, 2)}\n`);
    ElMessage.success("排除 tag 已导出");
  } catch (e) {
    ElMessage.error(`导出排除 tag 失败：${String(e)}`);
  }
}

async function importExcludedTags() {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title: "导入排除 tag",
      filters: [{ name: "Tag 列表", extensions: ["json", "txt", "csv"] }]
    });
    if (typeof selected !== "string" || !selected) return;
    const imported = parseExcludedTagImport(await readTextFile(selected));
    if (!imported.length) {
      ElMessage.warning("没有识别到可导入的 tag");
      return;
    }
    const before = getExcludedTags().length;
    setExcludedTags([...getExcludedTags(), ...imported]);
    ElMessage.success(`已导入 ${getExcludedTags().length - before} 个新排除 tag`);
  } catch (e) {
    ElMessage.error(`导入排除 tag 失败：${String(e)}`);
  }
}

async function exportLocalTranslations() {
  try {
    const path = await saveFile({
      title: "导出本地 tag 翻译",
      defaultPath: "pixiv-local-tag-translations.json",
      filters: [{ name: "JSON", extensions: ["json"] }]
    });
    if (!path) return;
    const payload = {
      type: "pixivLocalTagTranslations",
      version: 1,
      translations: getLocalTranslations()
    };
    await writeTextFile(path, `${JSON.stringify(payload, null, 2)}\n`);
    ElMessage.success("本地 tag 翻译已导出");
  } catch (e) {
    ElMessage.error(`导出本地 tag 翻译失败：${String(e)}`);
  }
}

async function importLocalTranslations() {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title: "导入本地 tag 翻译",
      filters: [{ name: "翻译表", extensions: ["json", "txt", "csv"] }]
    });
    if (typeof selected !== "string" || !selected) return;
    const imported = parseLocalTranslationImport(await readTextFile(selected));
    const count = Object.keys(imported).length;
    if (!count) {
      ElMessage.warning("没有识别到可导入的翻译项");
      return;
    }
    const before = Object.keys(getLocalTranslations()).length;
    setLocalTranslations({ ...getLocalTranslations(), ...imported });
    ElMessage.success(`已导入 ${Object.keys(getLocalTranslations()).length - before} 个新翻译项，已有项按导入文件覆盖`);
  } catch (e) {
    ElMessage.error(`导入本地 tag 翻译失败：${String(e)}`);
  }
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

// 自定义数据库路径走外部 JSON 配置（鸡生蛋，存不进 db 自身），
// 与 app_settings 不同表，因此单独 debounce 自动保存。
// 留空 → setCustomDbPath("") → external_config 删除自定义项回退到默认路径。
// 重启后生效，所以这里只刷 dbPathInfo，不打扰用户的"已保存"提示。
watchDebounced(
  () => customDbPath.value,
  async (next, prev) => {
    if (prev === undefined) return; // 初始挂载时同步赋值，跳过首次触发
    autoSaveState.value = "saving";
    try {
      await configStore.setCustomDbPath(next);
      autoSaveState.value = "saved";
    } catch (e: any) {
      autoSaveState.value = "error";
      ElMessage.error(e?.toString() ?? "数据库路径保存失败");
    }
  },
  { debounce: 600, maxWait: 2000 }
);
</script>

<template>
  <div class="settings-page">
    <div class="settings-layout">
      <!-- 左侧导航 -->
      <aside class="settings-nav">
        <button
          v-for="s in sections"
          :key="s.id"
          type="button"
          class="nav-item"
          :class="{ 'is-active': activeSection === s.id }"
          @click="scrollToSection(s.id)"
        >
          {{ s.label }}
        </button>

        <div class="nav-footer">
          <span v-if="autoSaveState === 'saving'" class="save-state">正在保存…</span>
          <span v-else-if="autoSaveState === 'saved'" class="save-state">已保存</span>
          <span v-else class="save-state is-error">保存失败</span>
        </div>
      </aside>

      <!-- 右侧内容（独立滚动） -->
      <div ref="scrollRef" class="settings-scroll ff-scroll">
        <div class="settings-content">
          <!-- 基础配置 -->
          <Panel data-section="basic" title="基础" :padded="false">
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
                <div class="flex-input db-path-group">
                  <el-input
                    v-model="configStore.settings.moveTargetPath"
                    placeholder="留空使用程序目录/temp_moved_files"
                    clearable
                  />
                  <el-button :icon="Folder" @click="pickMoveTargetFolder">选择目录</el-button>
                </div>
              </div>

              <div class="form-row">
                <label class="label">保留源目录结构</label>
                <div class="flex-input">
                  <el-switch v-model="configStore.settings.preserveDirOnMove" />
                  <span class="hint-inline">
                    开启后移动会按文件相对任务输入根的子路径建子目录。例如输入
                    D:\Game\test 时，D:\Game\test\unknown\foo.png 会落到
                    &lt;目标&gt;\&lt;taskId&gt;\unknown\foo.png；找不到匹配根的孤儿文件
                    自动降级为平铺。
                  </span>
                </div>
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
          <Panel data-section="performance" title="性能" :padded="false">
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
          <Panel data-section="preview" title="预览" :padded="false">
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
          <Panel data-section="tools" title="工具默认值" :padded="false">
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

              <div class="form-row">
                <label class="label">启用 Mod 操作回滚</label>
                <div class="flex-input">
                  <el-switch v-model="configStore.settings.modRollbackEnabled" />
                  <span class="hint-inline">仅作用于"重复删除 / 不同版本删除 / 移除版本限制"。关闭后这三类不再创建备份，记录管理页的"撤回"按钮置灰。</span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">Mod 备份目录</label>
                <div class="flex-input db-path-group">
                  <el-input
                    v-model="configStore.settings.modBackupDir"
                    placeholder="留空使用程序目录/mod-backups"
                    clearable
                    :disabled="!configStore.settings.modRollbackEnabled"
                  />
                  <el-button
                    :icon="Folder"
                    :disabled="!configStore.settings.modRollbackEnabled"
                    @click="pickModBackupFolder"
                  >
                    选择目录
                  </el-button>
                </div>
              </div>
            </div>
          </Panel>

          <!-- Pixiv 标签 -->
          <Panel data-section="pixiv" title="Pixiv 标签" :padded="false">
            <div class="form">
              <div class="form-row">
                <label class="label">获取标签接口地址</label>
                <div class="flex-input">
                  <el-input
                    v-model="configStore.settings.pixivTagApiBase"
                    placeholder="https://www.pixiv.net/ajax/illust/"
                  />
                  <span class="hint">最终请求 = 接口地址 + PID。建议保留默认值。</span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">使用英文译名显示</label>
                <div class="flex-input">
                  <el-switch v-model="configStore.settings.pixivUseTranslation" />
                  <span class="hint-inline">
                    Pixiv 响应里 `translation.en` 有值的 tag 会用译名替代原 tag 显示，
                    点击移动也按译名建子目录；缺译名的 tag 自动回落原 tag。
                    任务面板顶部"使用英文译名"开关与本设置同步。
                  </span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">每分钟最大请求数</label>
                <div class="flex-input">
                  <el-input-number
                    v-model="configStore.settings.pixivRateLimitPerMinute"
                    :min="1"
                    :max="600"
                    :step="10"
                    controls-position="right"
                  />
                  <span class="hint-inline">
                    防止被 Pixiv 拉黑：所有并发 worker / 重试 共享一条节流队列，
                    整体速率被锁在 `值/60` 次/秒。默认 60（每秒 1 条）已经相当保守；
                    游客身份下不建议高于 120，登录态 / 代理稳定时可上调到 300。
                  </span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">UI 刷新间隔（毫秒）</label>
                <div class="flex-input">
                  <el-input-number
                    v-model="configStore.settings.pixivPartialFlushIntervalMs"
                    :min="0"
                    :max="10000"
                    :step="100"
                    controls-position="right"
                  />
                  <span class="hint-inline">
                    后端拉取结果到达前端的合并刷新节奏。`0` = 实时（默认，每条结果立刻刷
                    chip 与状态）；`>0` = 节流，多个结果合并到一次 commit。
                    扫描几万张图时 `300–800` 能明显降低视觉抖动；不影响后端拉取速度。
                    `done` 终态会立刻 flush，统计不被节流拖延。
                  </span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">排除的 tag</label>
                <div class="flex-input">
                  <div class="field-toolbar">
                    <span class="count-pill">{{ getExcludedTags().length }} 个 tag</span>
                    <div class="field-actions">
                      <el-button size="small" :icon="Upload" @click="importExcludedTags">导入</el-button>
                      <el-button size="small" :icon="Download" @click="exportExcludedTags">导出</el-button>
                    </div>
                  </div>
                  <!--
                    自定义气泡输入:点击容器聚焦输入框,输入 ; / ; 自动拆成 chip;
                    Backspace 在空输入框上删最后一个;Enter / 失焦提交剩余 buffer。
                  -->
                  <div
                    class="chip-input"
                    @click="excludedInputRef?.focus()"
                  >
                    <el-tag
                      v-for="tag in getExcludedTags()"
                      :key="tag"
                      closable
                      :disable-transitions="true"
                      size="small"
                      @close="removeExcludedTag(tag)"
                    >
                      {{ tag }}
                    </el-tag>
                    <input
                      ref="excludedInputRef"
                      :value="excludedTagBuffer"
                      class="chip-input-field"
                      :placeholder="
                        getExcludedTags().length === 0
                          ? '输入 tag 名,多个用 ; 分隔;回车 / 失焦提交最后一个'
                          : ''
                      "
                      @input="onExcludedInput"
                      @keydown="onExcludedKeydown"
                      @blur="commitExcludedInput"
                    />
                  </div>
                  <span class="hint">
                    输入 tag 名,多个之间用半角 `;` 或全角 `；` 分隔,会自动变成气泡。
                    点 chip 上的 × 删除,光标在最末位置按 Backspace 也能删掉最后一个。
                    排除判断会同时匹配原 tag 与当前显示文本。
                  </span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">本地 tag 翻译</label>
                <div class="flex-input">
                  <div class="field-toolbar">
                    <span class="count-pill">{{ Object.keys(getLocalTranslations()).length }} 条翻译</span>
                    <el-input
                      v-model="localTranslationSearch"
                      size="small"
                      class="translation-search"
                      placeholder="搜索原 tag / 译名"
                      clearable
                    />
                    <div class="field-actions">
                      <el-button size="small" :icon="Upload" @click="importLocalTranslations">导入</el-button>
                      <el-button size="small" :icon="Download" @click="exportLocalTranslations">导出</el-button>
                    </div>
                  </div>

                  <div class="translation-editor">
                    <el-input
                      v-model="localTranslationKey"
                      placeholder="原 tag"
                      clearable
                    />
                    <el-input
                      v-model="localTranslationValue"
                      placeholder="本地译名"
                      clearable
                      @keydown.enter.prevent="upsertLocalTranslation"
                    />
                    <el-button type="primary" :icon="Plus" @click="upsertLocalTranslation">添加 / 更新</el-button>
                  </div>

                  <div v-if="localTranslationEntries.length" class="translation-list ff-scroll">
                    <div
                      v-for="[key, value] in localTranslationEntries"
                      :key="key"
                      class="translation-row"
                    >
                      <span class="translation-key" :title="key">{{ key }}</span>
                      <span class="translation-arrow">→</span>
                      <span class="translation-value" :title="value">{{ value }}</span>
                      <el-button
                        text
                        size="small"
                        type="primary"
                        :icon="Edit"
                        @click="editLocalTranslation(key, value)"
                      />
                      <el-button
                        text
                        size="small"
                        type="danger"
                        :icon="Delete"
                        @click="removeLocalTranslation(key)"
                      />
                    </div>
                  </div>
                  <span v-else class="hint">暂无本地翻译。开启"使用英文译名显示"后，本地译名会优先于 Pixiv 返回的 translation.en。</span>
                  <span class="hint">
                    导入 JSON 对象示例：{"{ \"コイカツ\": \"恋活\" }"}；也支持每行 `原 tag=译名`。
                    已有相同原 tag 时，导入内容会覆盖本地旧译名。
                  </span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">代理</label>
                <div class="flex-input">
                  <el-input
                    v-model="configStore.settings.pixivProxy"
                    placeholder="如 http://127.0.0.1:7890 或 socks5://127.0.0.1:1080；留空读环境变量"
                    clearable
                  />
                  <span class="hint">中国大陆访问 Pixiv 通常需要配代理；支持 HTTP / HTTPS / SOCKS5。</span>
                </div>
              </div>

              <div class="form-row">
                <label class="label">Pixiv Cookie</label>
                <div class="flex-input">
                  <el-input
                    v-model="configStore.settings.pixivCookie"
                    type="textarea"
                    :rows="3"
                    placeholder="完整 Cookie 字符串（如 PHPSESSID=...; ...）；留空使用游客身份，部分 tag 取不到"
                    clearable
                  />
                  <span class="hint">仅在本机数据库与配置中保存，不会上传。</span>
                </div>
              </div>
            </div>
          </Panel>

          <!-- 数据库管理 -->
          <Panel data-section="database" title="数据库管理" :padded="false">
            <div class="form">
              <div class="form-row">
                <label class="label">当前数据库路径</label>
                <el-input
                  :model-value="stripWindowsExtendedPrefix(configStore.dbPathInfo?.currentPath ?? '')"
                  disabled
                  class="flex-input"
                />
              </div>

              <div class="form-row">
                <label class="label">自定义数据库路径</label>
                <div class="flex-input">
                  <div class="db-path-group">
                    <el-input
                      v-model="customDbPath"
                      placeholder="留空使用默认目录"
                      clearable
                    />
                    <el-button :icon="Folder" @click="pickDbFolder">选择目录</el-button>
                  </div>
                  <div class="hint">修改后重启应用生效</div>
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
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  height: 100%;
  min-height: 0;
  overflow: hidden;
}

.settings-layout {
  height: 100%;
  display: grid;
  grid-template-columns: 180px minmax(0, 1fr);
  gap: var(--ff-space-4);
  max-width: 1200px;
  margin: 0 auto;
}

/* ---- 左侧导航 ---- */
.settings-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--ff-space-3) var(--ff-space-2);
  border-right: 1px solid var(--ff-border-subtle);
  min-width: 0;
}

.nav-item {
  appearance: none;
  background: transparent;
  border: 0;
  text-align: left;
  color: var(--ff-text-secondary);
  padding: 8px 12px;
  font-size: var(--ff-font-md);
  font-weight: 500;
  border-radius: var(--ff-radius-sm);
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.nav-item:hover:not(.is-active) {
  background: var(--ff-bg-muted);
  color: var(--ff-text-primary);
}
.nav-item.is-active {
  background: var(--ff-accent-soft);
  color: var(--ff-accent);
  font-weight: 600;
}

.nav-footer {
  margin-top: auto;
  padding: var(--ff-space-2) var(--ff-space-3);
  border-top: 1px solid var(--ff-border-subtle);
}
.save-state {
  font-size: var(--ff-font-xs);
  color: var(--ff-text-muted);
}
.save-state.is-error {
  color: var(--ff-danger);
}

/* ---- 右侧滚动容器 ---- */
.settings-scroll {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  min-height: 0;
}
.settings-content {
  display: flex;
  flex-direction: column;
  gap: var(--ff-space-4);
  padding: var(--ff-space-3) var(--ff-space-2) var(--ff-space-6);
}

/* ---- 表单 ---- */
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
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.db-path-group .el-input {
  flex: 1;
  min-width: 200px;
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

.field-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}
.field-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-left: auto;
}
.count-pill {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 8px;
  border-radius: var(--ff-radius-sm);
  background: var(--ff-bg-muted);
  color: var(--ff-text-muted);
  font-size: var(--ff-font-xs);
}
.translation-search {
  max-width: 220px;
}
.translation-editor {
  display: grid;
  grid-template-columns: minmax(160px, 1fr) minmax(160px, 1fr) auto;
  gap: 8px;
}
.translation-list {
  max-height: 220px;
  overflow: auto;
  border: 1px solid var(--ff-border-subtle);
  border-radius: var(--ff-radius-sm);
  background: var(--ff-bg-panel);
}
.translation-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 8px;
  min-height: 34px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--ff-border-subtle);
}
.translation-row:last-child {
  border-bottom: 0;
}
.translation-key,
.translation-value {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.translation-key {
  color: var(--ff-text-secondary);
}
.translation-value {
  color: var(--ff-text-primary);
  font-weight: 500;
}
.translation-arrow {
  color: var(--ff-text-muted);
  font-size: var(--ff-font-xs);
}

/* ---- 排除 tag 的气泡输入容器 ---- */
.chip-input {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  min-height: 32px;
  padding: 4px 8px;
  border: 1px solid var(--el-border-color);
  border-radius: var(--el-border-radius-base);
  background: var(--el-fill-color-blank);
  cursor: text;
  transition: border-color 0.15s, box-shadow 0.15s;
}
.chip-input:hover {
  border-color: var(--el-border-color-hover);
}
.chip-input:focus-within {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 2px var(--el-color-primary-light-9, rgba(64, 158, 255, 0.1));
}
.chip-input-field {
  flex: 1 1 120px;
  min-width: 80px;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--ff-text-primary, var(--el-text-color-primary));
  font: inherit;
  font-size: var(--el-font-size-base);
  padding: 2px 0;
}
.chip-input-field::placeholder {
  color: var(--el-text-color-placeholder);
}

@media (max-width: 800px) {
  .settings-layout {
    grid-template-columns: 1fr;
  }
  .settings-nav {
    flex-direction: row;
    overflow-x: auto;
    border-right: 0;
    border-bottom: 1px solid var(--ff-border-subtle);
    padding: var(--ff-space-2);
    flex-wrap: nowrap;
  }
  .nav-footer {
    margin-top: 0;
    margin-left: auto;
    padding: 0 var(--ff-space-2);
    border-top: 0;
  }
  .form-row {
    grid-template-columns: 1fr;
    gap: var(--ff-space-1);
  }
  .field-actions {
    margin-left: 0;
  }
  .translation-editor {
    grid-template-columns: 1fr;
  }
  .translation-row {
    grid-template-columns: minmax(0, 1fr) auto;
  }
  .translation-arrow,
  .translation-value {
    display: none;
  }
}
</style>
