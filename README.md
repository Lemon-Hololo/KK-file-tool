# KK File Tool

基于 Tauri 2 的 Windows 桌面文件处理平台。围绕「批量整理 + 全程可撤回」的工作流，把日常本地资源管理里反复出现的几件事打包成一站式工具：文件去重、后缀批量修改、空文件夹清理、`.zipmod` 工具、Pixiv 标签整理、文件预览、统一记录管理。

- 应用名 / 版本：`kk-file-tool` v1.0.1
- 应用 ID：`com.holo.kk-file-tool`
- 默认窗口：1440 × 900（最小 1200 × 760）
- 平台：Windows（重度依赖 `\\?\` 长路径前缀与 NSIS / WiX 中文安装包）

完整的代码风格、架构约定与协作规范见 [CLAUDE.md](./CLAUDE.md)，本 README 只做功能与上手层面的说明。

---

## 核心功能

### 1. 文件去重

- BLAKE3 计算文件哈希识别重复文件。
- 流式扫描 → 哈希 → 分组流水线，结果通过 `task_result_partial` 增量推送到前端，大目录不必等扫描全部结束才能看到分组。
- 支持暂停 / 恢复 / 停止；可勾选「使用上次哈希记录」做增量复用。
- 选中重复文件可移动到统一目标目录，跨盘自动 fallback 到 copy + delete；移动结束生成完整的成功 / 失败 / 跳过分类报告。
- 哈希记录入库，后续可单独管理、重命名、删除、应用。

### 2. 后缀批量修改

- 输入「目标后缀」对一批文件批量改扩展名。
- 标准的预览 → 应用 → 撤回流程，全程留痕，可在「记录管理」页随时回滚。
- 撤回时若原路径已被外部新文件占用，自动追加 ` (1)` / ` (2)` 后缀避让，绝不静默覆盖用户后写入的文件。

### 3. 空文件夹清理

- 递归扫描指定目录下所有空文件夹，按深度从深到浅删除。
- 单条输入路径访问失败（不存在 / 无权限 / 不是目录）只跳过该条，不会因此 abort 整批扫描；只有全部输入路径都失败才返回错误。
- 撤回会重新创建被删除的空目录；任务输入根目录默认不删除以保护用户原始结构。

### 4. Mod 工具

围绕 `manifest.xml` 元数据展开的五个独立子工具：

| 子功能 | 作用 |
|--------|------|
| 重命名 | 按 `[author] guid-version.zipmod` 规范化文件名；同批次撞名按稳定顺序自动追加 ` (N)` 冲突后缀 |
| 归类 | 按文件名首个 `[...]` 创建子目录归位 |
| 重复 MOD 检查 | 按 `guid + author + version` 分组找重复，默认每组保留最新 |
| 不同版本 MOD 检查 | 按 `guid + author` 分组找多个 `version`，默认保留最高版本 |
| 移除版本限制 | 就地重写 zip 去掉指定 `<game>KEYWORD</game>` 标签，manifest 之外的条目零重编码复制 |
| 版本限制扫描 | 长任务，扫描全部含限制标签的 zipmod；勾选后由「移除版本限制」批量落盘 |

- 「重复删除 / 不同版本删除 / 移除版本限制」是**备份型操作**，受 `mod_rollback_enabled` 控制：默认开启，备份到 `<backup_root>/<record_id>/<原文件名>`，记录主表 `rollback_enabled = 1`，可撤回。
- 关闭备份后这三类直接 `remove_file` / 原地改写不留痕；记录 `rollback_enabled = 0`，撤回按钮置灰、后端命令也会拒绝。
- 备份目录默认 `<exe_dir>/mod-backups`，可在配置中心自定义；与源文件跨卷时自动 fallback 到 copy + delete。
- 重命名 / 归类不进入备份概念，永远可撤回。

### 5. 文件预览

- 文本预览：UTF-8 / GBK / Shift_JIS 自动回退编码；最大字节数可配置（默认 256 KiB）。
- 图片预览：含尺寸、格式等元信息。
- 压缩包预览：枚举条目路径、大小、目录标记、修改时间；最大枚举条数可配置（默认 5000）。
- 任务面板路径列 hover 即弹出预览面板，无需点开。

### 6. Pixiv 标签整理

- 按文件名中的 8~9 位 PID 调用 `https://www.pixiv.net/ajax/illust/<pid>` 获取 tag。
- 每条 tag 渲染为 chip 气泡；点击 chip 把图移动到 `<输出目录>/<tag>/`，行保留以便往别的 tag 文件夹继续移。
- 同 PID 多张图（`_p0..._pN`）支持组级共享 tag；行级提供三态 segmented 控件（`global` / `original` / `translated`）单独覆盖译名策略。
- 配置项：
  - 接口 base URL（兼容反代）
  - 排除 tag（不渲染为 chip）
  - 本地 tag 翻译表（`原tag → 译名`，优先于 Pixiv 响应里的 `translation.en`）
  - Pixiv Cookie（解锁 R-18 / 关注限定 tag，仅本机保存）
  - HTTP / HTTPS / SOCKS5 代理 URL
  - 全局译名开关
  - 每分钟最大请求数（限速队列在所有并发 worker 与单条重试之间共享，整体速率被钉死在「值 / 60」次 / 秒）
  - partial 增量结果合并刷新间隔
- 排除 tag 与本地翻译表都支持 JSON / 文本文件导入、JSON 导出。
- 顶部「重试失败 (N)」按钮按 PID 去重后串行重试所有 error 行，复用同一条限速队列。
- HTTP 客户端走后端 `reqwest`（rustls + webpki bundled 根证书），规避 CORS 与系统证书库依赖。

### 7. 记录管理

- 哈希记录、后缀记录、空文件夹清理记录、Mod 操作记录统一在「记录管理」页。
- Mod 记录按 `kind` 分 `rename` / `organize` / `modify` / `duplicate_delete` / `version_delete` 五个 tab。
- 详情抽屉展示完整 item 列表；支持单条 / 批量撤回与删除。
- 撤回过程对外部冲突有保护：自动避让，不会覆盖用户后来放进同名位置的新文件。

---

## 技术栈

### 前端

| 技术 | 版本 | 用途 |
|------|------|------|
| Vue 3 | ^3.5 | Composition API + `<script setup>` |
| TypeScript | ^5.9 | 严格模式 |
| Pinia | ^3.0 | Options API 风格 store |
| Element Plus | ^2.13 | 表单控件（卡片 / 段式 tab / 滚动条改用项目自有原语） |
| VueUse | ^14.2 | `useStorage` / `useDark` / `useElementSize` 等 |
| Vue Router | ^5.0 | Hash 模式 |
| Vite | ^7.3 | 构建 |
| @tauri-apps/api | ^2.10 | IPC |
| @tauri-apps/plugin-dialog | ^2.6 | 原生对话框 |

### 后端（Rust）

| 技术 | 版本 | 用途 |
|------|------|------|
| Tauri | 2.10 | 桌面框架（开启 `protocol-asset`） |
| tokio | 1.49 | 异步运行时（rt-multi-thread / macros / sync / time） |
| rusqlite | 0.38 | SQLite（bundled + WAL） |
| blake3 | 1.8 | 文件哈希 |
| walkdir | 2.5 | 递归遍历 |
| rayon | 1.10 | 并行迭代（按操作本地化线程池，避免污染全局池） |
| zip | 8.1 | 压缩包读取与零重编码重写 |
| quick-xml | 0.39 + encoding_rs 0.8 | manifest.xml 解析与编码回退（UTF-8 / GBK / Shift_JIS） |
| image | 0.25 | 图片元信息预览 |
| reqwest | 0.13 | Pixiv tag HTTP 客户端（rustls + gzip + json + socks，无 OpenSSL 依赖） |
| regex | 1 | Pixiv 文件名 PID 提取（`\d{8,9}`） |
| chrono / uuid / num_cpus / serde / thiserror | — | 通用基础设施 |

---

## 快速开始

### 环境要求

- Node.js 18+ 与 npm。
- Rust 1.75+（含 cargo），按 [Tauri 2 官方指南](https://v2.tauri.app/start/prerequisites/) 安装 Windows 平台依赖（MSVC build tools、WebView2 等）。

### 安装依赖

```bash
npm install
```

### 开发模式

```bash
npm run tauri dev
```

首次启动会下载并编译全部 Rust 依赖，需要等几分钟；之后增量编译。

### 构建发行版

```bash
npm run tauri build
```

产物输出至 `src-tauri/target/release/bundle/`（默认 NSIS / MSI 安装包，WiX 中文界面）。

### 类型与编译双把关

任何代码改动后都跑这两条命令再做 UI 验证：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
npx vue-tsc --noEmit
```

---

## 目录结构

```
KK-file-tool/
├── src/                            前端
│   ├── App.vue / main.ts
│   ├── router/                     hash 路由 3 条
│   ├── views/                      TaskPage / SettingsPage / RecordManagePage
│   ├── components/
│   │   ├── DedupPanel.vue              去重面板（独立，不走 OpsPanel）
│   │   ├── SuffixPanel / EmptyDirsPanel /
│   │   │ ModRenamePanel / ModOrganizePanel
│   │   │                               OpsPanel 薄包装（< 100 行）
│   │   ├── ModDuplicatePanel / ModVersionPanel
│   │   │                               ModGroupPanel kind 薄包装
│   │   ├── ModGroupPanel / ModScanPanel / ModToolsPanel
│   │   ├── PixivTagPanel / RealtimeLogPanel
│   │   ├── DuplicateGroupTable / PreviewPanel / PathPreviewLink
│   │   ├── MoveConfirmDialog / MoveReportDialog
│   │   ├── RecordTab / RecordDetailDrawer / TaskControlPanel
│   │   └── common/
│   │       ├── Panel.vue               替代 el-card
│   │       ├── TabBar.vue              替代 el-tabs（v-show 切换零 reflow）
│   │       ├── VirtualTable.vue        通用虚拟滚动表
│   │       └── OpsPanel.vue            预览 → 应用 → 撤回 通用面板
│   ├── stores/                     Pinia Options 风格（含 _opRecordCrud 工厂）
│   ├── services/                   IPC 封装
│   ├── types/ composables/ utils/ constants/
│   └── styles/index.css            设计令牌 + 暗色主题
├── src-tauri/src/                  后端
│   ├── main.rs / lib.rs            入口 + 命令注册
│   ├── app_state.rs                AppState + TaskRuntime
│   ├── constants.rs                事件名 / 阶段 / 枚举字符串集中
│   ├── config.rs                   编译期常量
│   ├── error.rs                    AppError / AppResult
│   ├── external_config.rs          数据库路径外部 JSON
│   ├── models/                     按领域拆分的 DTO，全部 camelCase serde
│   ├── db/
│   │   ├── op_record_repo.rs       通用「操作记录」仓储
│   │   ├── hash_repo / move_repo / settings_repo
│   │   └── schema.rs               CREATE IF NOT EXISTS / ALTER ADD COLUMN
│   ├── services/
│   │   ├── op_pipeline.rs          通用 preview→apply→rollback 流水线
│   │   ├── events.rs / logging.rs
│   │   ├── suffix / empty_dirs / dedup / move_file / preview / pixiv_tag
│   │   └── mod_tools/              rename / organize / cleanup / modify / scan / backup / zipmod
│   ├── commands/                   纯转发的 #[tauri::command]
│   └── utils/                      path / hash / filename / time
├── README.md                       本文件
└── CLAUDE.md                       项目完整规范（架构 / 风格 / 协作）
```

---

## 架构亮点

### 通用「操作记录」抽象（op_record / op_pipeline / OpsPanel）

后缀修改、空文件夹清理、Mod 重命名 / 归类 / 重复删除 / 旧版本删除 / 修改版本限制，本质都是：

> 产生一批 `(old_path, new_path)` → 批量执行 → 写入可回滚记录

项目把这套模式抽成三层，**任何同类业务都必须走这套抽象**：

- **`db::op_record_repo`**：通用记录 CRUD，通过 `OpRecordTables` 描述符传入表名与附加列。item 表硬约束 `id / record_id / old_path / new_path / apply_success / apply_error / rollback_success / rollback_error / updated_at`。
- **`services::op_pipeline`**：
  - 统一并发控制（`resolve_thread_count` / `resolve_io_concurrency_multiplier` / `rayon_pool`）
  - 跨卷自动 fallback 的 `rename_or_copy_delete`
  - `persist_apply_rename_pairs`（纯 rename）/ `persist_apply_with_executor`（自定义闭包，如 modify 的「备份 + 重写」）
  - `check_rollback` / `rollback`，撤回时用 `resolve_conflict_with_reserved` 保证同批次冲突避让
- **`OpsPanel.vue`**：泛型面板，调用方只需提供列定义、`rows` computed、`preview` / `apply` / `checkRollback` / `rollback` 四个回调，子组件 < 100 行。
- **`stores/_opRecordCrud.ts`**：`createOpRecordCrudActions` / `createOpRecordCrudActionsWithRename` 两个 action 工厂，统一生成记录型 store 的 `refreshRecords / loadDetail / checkRollback / rollback / remove / removeBatch / rename`。

### Windows 长路径
内部 `std::fs` 调用一律加 `\\?\` 前缀（`to_extended_length_path`），返回前端一律去前缀（`to_user_friendly_path` / `stripWindowsExtendedPrefix`）。

### 高频事件批量化
单条 `task_log` IPC 高频时由前端 `runtime` store 写入非响应式缓冲，每 150 ms 批量刷入响应式 `logs`，`LOG_MAX_LENGTH = 3000` 自动裁剪旧数据。去重 partial 结果按 `PARTIAL_BATCH_SIZE = 30` 合批推送。

### 自有 UI 原语（替代 el-card / el-tabs / el-scrollbar）
- `Panel`：自控结构 header / body（`flex: 1; min-height: 0`）/ footer，VirtualTable auto-height 直接生效，不再 `:deep()` 覆盖 EP 内部类名。
- `TabBar`：段式控件，配合 `v-show` 切换内容零 reflow，避免 `el-tabs` 的 `display: none / block` 反复 fire ResizeObserver 触发滚动条闪烁。
- `VirtualTable`：手写虚拟滚动，单滚动容器，`scrollbar-gutter: stable` 杜绝纵向滚动条出现/消失反复重算列宽闪烁；支持列宽拖拽 / 全选 / 客户端或服务端分页 / 固定列 / ellipsis + tooltip / 列自定义（显示 / 左固定 / 顺序，持久化到 localStorage）/ 行高内容自适应 / 双击复制单元格文本。
- `OpsPanel`：标准的预览 → 应用 → 撤回交互。

---

## 配置项

「配置中心」页可调，自动保存到 SQLite `app_settings` 表。

| 字段 | 默认 | 说明 |
|------|------|------|
| `keep_policy` | `newest` | 去重 / 重复 MOD / 不同版本默认保留策略 |
| `move_target_path` | null | 重复文件移动目标目录 |
| `save_record_enabled` | true | 哈希索引是否入库 |
| `use_last_record_enabled` | false | 去重时是否复用上次哈希记录 |
| `include_current_folder_duplicates` | true | 是否统计当前目录内重复 |
| `theme_mode` | `system` | `light` / `dark` / `system` |
| `thread_count` | 0 | 并发线程数；0 = num_cpus |
| `log_max_length` | 3000 | 前端日志保留条数 |
| `io_concurrency_multiplier` | 2 | IO 并发倍率（× 有效线程数）；SSD/NVMe 可上调 4–8，HDD 压到 1 |
| `extreme_row_threshold` | 20000 | 虚拟表极限模式阈值 |
| `text_preview_max_kb` | 256 | 文本预览最大字节（KiB） |
| `zip_preview_max_entries` | 5000 | 压缩包预览枚举上限 |
| `mod_scan_default_keyword` | `Koikatsu` | Mod 扫描关键字默认值 |
| `suffix_default_target` | `txt` | 后缀修改默认目标（不带点） |
| `mod_rollback_enabled` | true | Mod 备份型操作（重复删除 / 不同版本删除 / 移除版本限制）是否创建备份 |
| `mod_backup_dir` | null | Mod 备份根目录；为空时使用 `<exe_dir>/mod-backups` |
| `pixiv_tag_api_base` | `https://www.pixiv.net/ajax/illust/` | Pixiv tag 接口 base URL；末尾斜杠可有可无 |
| `pixiv_excluded_tags` | `[]` | 不渲染为 chip 的 tag |
| `pixiv_local_tag_translations` | `{}` | 本地 tag 翻译表（优先于 `translation.en`） |
| `pixiv_cookie` | null | Pixiv Cookie（仅本机保存） |
| `pixiv_proxy` | null | HTTP / HTTPS / SOCKS5 代理 URL |
| `pixiv_use_translation` | false | 是否在 chip 上用译名替代原 tag |
| `pixiv_rate_limit_per_minute` | 60 | 每分钟最大请求数（0 = 不限速；UI 限制 1–600） |
| `pixiv_partial_flush_interval_ms` | 0 | partial 合并刷新间隔（ms），0 = 实时；50K 张图配 300–800 ms 明显降抖 |

数据库路径外部存于 `<app_data_dir>/kk-file-tool_config.json`（鸡生蛋问题——无法把 db 路径存进 db 本身），后端解析时自动兜底「目录追加 `kk-file-tool.db`」。

---

## 注意事项

- **平台限定**：仅 Windows。代码大量依赖 `\\?\` 长路径前缀、NSIS / WiX 安装包；其他平台未适配。
- **数据库**：SQLite + WAL 模式，按需打开连接（无连接池）。所有迁移仅 `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE ADD COLUMN`，不改已有列。
- **Pixiv 接口**：中国大陆访问通常需要配置代理或自定义反代 base URL；reqwest 已 bundled rustls 根证书，无系统证书库依赖。
- **Mod 备份目录**：建议放到容量大的数据盘分区。与源 `.zipmod` 跨卷时由 `op_pipeline::rename_or_copy_delete` 自动 copy + delete 兜底，不会丢文件。
- **撤回保护**：apply 之后又往原目录放了同名文件、再点撤回时，已存在的文件不会被静默覆盖——撤回回来的文件会落到 `<原文件名> (N).<ext>`。
- **删除数据库 / 切换数据库**：受 `AppState::has_active_tasks()` 严格判定保护，只要任务尚未走完终态收尾（包括取消后的备份目录 rename / 哈希记录落盘等）都视为活跃，操作会被拒绝。

---

## 协作约定（要点摘录）

完整规范见 [CLAUDE.md](./CLAUDE.md)，修改代码前请先读完。

1. **前后端模型同步**：`models/*.rs` ↔ `types/*.ts`，全部 camelCase。
2. **命令注册**：新 `#[tauri::command]` 必须在 `lib.rs::invoke_handler!` 注册并写前端 `services/<feature>.ts`。
3. **记录型操作**：后端走 `op_record_repo + op_pipeline`，任务面板挂 `OpsPanel` 薄包装，记录管理走 `RecordTab`，前端 store 用 `stores/_opRecordCrud.ts` 工厂生成 CRUD。**严禁复制 suffix / mod_tools 当模板**。
4. **路径**：`std::fs` 调用一律 `to_extended_length_path`；返回前端一律 `to_user_friendly_path`。
5. **文件名工具**：用 `utils::filename` 里的 `split_name_ext` / `resolve_conflict_with_reserved` / `normalize_suffix` 等；构造 `(old, new)` 对必须用 `resolve_conflict_with_reserved` + `HashSet`，避免同批次解析到同一目标互相覆盖。
6. **并发**：线程数从 `op_pipeline::resolve_thread_count(db_path)` 取，rayon 池用 `op_pipeline::rayon_pool`，**禁止 inline 读 `settings.thread_count` 或污染全局池**。
7. **长任务终态**：spawn 失败兜底统一用 `events::finalize_failed_long_task`，启动用 `events::spawn_long_task`，不要在每个命令模块再抄一份「emit_state_changed + emit_task_failed + remove_task」三件套。
8. **注释**：写 WHY（非直觉约束、调用契约、并发顺序），不写 WHAT。新的 `pub fn` / `pub struct` / `pub enum` 必须 `///`；新的 TS `export function` / store action 必须 JSDoc。
9. **UI**：表单控件用 Element Plus，结构容器用 `Panel` / `TabBar` / `VirtualTable` / `OpsPanel`；**不再使用 `el-card` / `el-tabs` / `el-scrollbar`**。
10. **同步文档**：跨模块改动必须同步更新 [CLAUDE.md](./CLAUDE.md) 对应段落。
