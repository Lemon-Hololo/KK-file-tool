# KK File Tool — 项目规范

本文档是 KK File Tool 的架构与风格规范，也是 AI 辅助开发时的 single source of truth。修改代码前先读，修改代码后必须同步更新。

---

## 0. 文档维护

- 本文档描述**当前代码的真实状态**，不是路线图。任何不符合现状的描述都是 bug，应立即修正。
- 以下改动**必须同步更新本文档**：
  - 新增 / 删除 / 重命名模块、数据库表、Tauri 命令、事件、常量；
  - 改动公共抽象（`op_pipeline` / `op_record_repo` / `OpsPanel` / `VirtualTable` 等）的 API 或调用契约；
  - 改动并发模型、路径处理、注释规范等跨模块约定；
  - 调整目录结构或分层语义。
- **以当前状态为准直接覆写**对应段落，不要留"已删除 / 已废弃 / 新增"之类的补丁语气。

---

## 1. 项目概述

基于 Tauri 2 的 Windows 桌面文件处理平台，主要功能：

- **文件去重**：BLAKE3 哈希识别重复文件，支持暂停/恢复/停止与增量移动。
- **后缀批量修改**：批量改扩展名，支持预览 / 应用 / 撤回 / 历史记录。
- **空文件夹清理**：递归预览空目录，按深度从深到浅删除并写入可撤回记录；撤回会重新创建空目录，默认不删除任务输入根目录。
- **Mod 工具**：针对 Illusion 系列 `.zipmod` 的六类操作——
  - **重命名**：按 manifest.xml 的 `guid/author/version` 生成 `[author] guid-version.zipmod`；同批次撞名时按稳定顺序自动分配 ` (N)` 冲突后缀。
  - **归类**：按文件名首个 `[...]` 建子目录归类。
  - **重复 MOD 检查**：按 `guid + author + version` 分组找重复，默认每组保留最新；删除移动到 `.kk-file-tool-del-*` 备份路径并可撤回。
  - **不同版本 MOD 检查**：按 `guid + author` 分组找多个 `version`，默认保留最高版本；同样备份+可撤回（`.kk-file-tool-del-*`）。
  - **移除版本限制（modify）**：就地重写 zip，从 manifest.xml 去掉指定 `<game>KEYWORD</game>` 标签，原文件备份（`.kk-file-tool-bak-*`）。
  - **版本限制扫描**：长任务，扫描结果勾选后由"移除版本限制"落盘为 Mod 操作记录。
- **文件预览**：文本 / 图片 / 压缩包内容查看；压缩包预览列出条目路径、大小、目录标记、修改时间。
- **记录管理**：哈希记录、后缀记录、空文件夹清理记录、Mod 操作记录统一在"记录管理"页；Mod 记录按 `kind` 分 `rename` / `organize` / `modify` / `duplicate_delete` / `version_delete`。

---

## 2. 技术栈

### 前端

| 技术 | 版本 | 用途 |
|------|------|------|
| Vue 3 | ^3.5 | Composition API + `<script setup>` |
| TypeScript | ^5.9 | 严格模式 |
| Pinia | ^3.0 | Options API 风格 store |
| Element Plus | ^2.13 | UI 组件库 |
| VueUse | ^14.2 | useStorage / useDark / useElementSize 等 |
| Vue Router | ^5.0 | Hash 模式 |
| Vite | ^7.3 | 构建 |
| @tauri-apps/api | ^2.10 | IPC |
| @tauri-apps/plugin-dialog | ^2.6 | 原生对话框 |

### 后端（Rust）

| 技术 | 版本 | 用途 |
|------|------|------|
| Tauri | 2.10 | 桌面框架（protocol-asset） |
| tokio | 1.49 | 异步运行时（rt-multi-thread, macros, sync, time） |
| rusqlite | 0.38 | SQLite（bundled + WAL） |
| blake3 | 1.8 | 文件哈希 |
| walkdir | 2.5 | 递归遍历 |
| rayon | 1.10 | 并行迭代；用 `ThreadPoolBuilder::install` 局部化避免污染全局池 |
| quick-xml | 0.36 | manifest.xml 解析 |
| encoding_rs | 0.8 | manifest 编码回退（UTF-8 / GBK / Shift_JIS） |
| zip | 8.1 | 压缩包读取、`raw_copy_file` 零重编码重写 |
| image | 0.25 | 图片元信息预览 |
| uuid | 1.21 | UUID v4 |
| chrono | 0.4 | 时间处理 |
| num_cpus | 1.17 | CPU 核心数 |
| serde / serde_json | 1 | 序列化，`rename_all = "camelCase"` |
| thiserror | 2.0 | `AppError` 派生 |

---

## 3. 目录结构

```
fileflow-desktop/                        # 仓库目录名（可手动改为 kk-file-tool）
├── src/                                 # 前端
│   ├── App.vue / main.ts
│   ├── router/{index,routes}.ts
│   ├── views/
│   │   ├── TaskPage.vue                 # 任务中心：路径+日志 / 去重 / 后缀 / 空文件夹 / Mod 工具
│   │   ├── SettingsPage.vue             # 配置中心（左侧导航 + 右侧滚动 + IntersectionObserver 高亮）
│   │   └── RecordManagePage.vue         # 四类记录管理（Mod 记录按 kind 过滤）
│   ├── components/
│   │   ├── DedupPanel.vue               # 去重面板（独立，不走 OpsPanel）
│   │   ├── SuffixPanel.vue              # 薄包装 → OpsPanel
│   │   ├── EmptyDirsPanel.vue           # 薄包装 → OpsPanel
│   │   ├── ModRenamePanel.vue           # 薄包装 → OpsPanel
│   │   ├── ModOrganizePanel.vue         # 薄包装 → OpsPanel
│   │   ├── ModDuplicatePanel.vue        # 重复 MOD 分组 + 删除（写 Mod 操作记录）
│   │   ├── ModVersionPanel.vue          # 不同版本 MOD 分组 + 删除（写 Mod 操作记录）
│   │   ├── ModScanPanel.vue             # 扫描 + 勾选 + modify
│   │   ├── ModToolsPanel.vue            # 五个 Mod 子 Tab 容器（TabBar + v-show）
│   │   ├── RealtimeLogPanel.vue         # 手写虚拟滚动日志
│   │   ├── DuplicateGroupTable.vue
│   │   ├── PreviewPanel.vue / MoveConfirmDialog.vue / MoveReportDialog.vue
│   │   ├── RecordDetailDrawer.vue / TaskControlPanel.vue
│   │   └── common/
│   │       ├── Panel.vue                # 自有卡片原语，替代 el-card
│   │       ├── TabBar.vue               # 自有段式切换，替代 el-tabs
│   │       ├── VirtualTable.vue         # 通用虚拟表
│   │       └── OpsPanel.vue             # 通用"预览→应用→撤回"面板
│   ├── stores/                          # Pinia Options-style
│   │   └── runtime.ts / task.ts / record.ts / config.ts / preview.ts /
│   │       suffix.ts / emptyDirs.ts / modTools.ts
│   ├── services/                        # IPC 封装
│   │   └── tauri.ts / task.ts / settings.ts / record.ts / preview.ts /
│   │       suffix.ts / emptyDirs.ts / modTools.ts
│   ├── types/
│   │   ├── common.ts / task.ts / settings.ts / record.ts / moveReport.ts /
│   │   │   preview.ts / virtualTable.ts
│   │   ├── opRecord.ts                  # 通用可撤回操作记录类型
│   │   └── suffix.ts / emptyDirs.ts / modTools.ts   # 基于 opRecord 特化
│   ├── composables/
│   │   ├── useTheme.ts                  # 主题切换
│   │   └── usePathNormalize.ts          # 路径规范化 + 警告弹窗
│   ├── utils/
│   │   ├── path.ts                      # uniquePaths / stripWindowsExtendedPrefix
│   │   ├── format.ts / error.ts / clipboard.ts
│   │   └── mapper.ts                    # dedup 分组前端包装透传
│   └── constants/
│       └── app.ts / task.ts / theme.ts / preview.ts
│
├── src-tauri/src/                       # 后端
│   ├── main.rs / lib.rs                 # 入口 + 命令注册
│   ├── app_state.rs                     # AppState + TaskRuntime
│   ├── constants.rs                     # 事件名 / 阶段 / 枚举字符串集中
│   ├── config.rs                        # 编译期常量
│   ├── error.rs                         # AppError + AppResult
│   ├── external_config.rs               # 启动前可读的 JSON 配置（数据库路径）
│   ├── models/                          # 按领域拆分的 DTO，全部 camelCase serde
│   │   └── mod.rs / task.rs / hash_record.rs / move_file.rs / path_norm.rs /
│   │       settings.rs / suffix.rs / empty_dirs.rs / mod_tools.rs
│   ├── db/                              # 数据访问层
│   │   ├── mod.rs / schema.rs
│   │   ├── op_record_repo.rs            # 通用"操作记录"仓储（suffix / mod_op / empty_dir 共用）
│   │   └── hash_repo.rs / move_repo.rs / settings_repo.rs
│   ├── services/                        # 业务逻辑
│   │   ├── mod.rs / events.rs / logging.rs
│   │   ├── op_pipeline.rs               # 通用 preview→apply→rollback 流水线
│   │   ├── suffix.rs / empty_dirs.rs / dedup.rs / move_file.rs / preview.rs
│   │   └── mod_tools/
│   │       ├── mod.rs                   # 记录查询/删除/重命名/撤回（映射到 op_pipeline）
│   │       ├── rename.rs / organize.rs  # 纯 rename → op_pipeline::persist_apply_rename_pairs
│   │       ├── cleanup.rs               # 重复/不同版本检查；分块并行解析 manifest，删除移动到备份路径
│   │       ├── modify.rs                # 非纯 rename → op_pipeline::persist_apply_with_executor
│   │       └── scan.rs / zipmod.rs
│   ├── commands/                        # #[tauri::command]，纯转发
│   │   └── mod.rs / path.rs / dedup.rs / runtime.rs / move_file.rs /
│   │       preview.rs / settings.rs / records.rs /
│   │       suffix.rs / empty_dirs.rs / mod_tools.rs
│   └── utils/
│       └── mod.rs / path.rs / hash.rs / filename.rs
│           # filename: split_name_ext / resolve_conflict / strip_conflict_suffix /
│           #          normalize_suffix / extract_bracket / sanitize_filename / normalize_brackets
```

---

## 4. 架构分层

```
前端 invokeCmd(name, args) ──▶ Tauri IPC ──▶ commands::<name>
                                              │   │
                                              │   ▼
                                              │  services::<feature>  ◀─▶ services::events (emit back)
                                              │   │
                                              │   ▼
                                              │  db::<feature>_repo / op_record_repo
                                              │   │
                                              │   ▼
                                              └─▶ SQLite / 文件系统
```

- **commands**：解析参数、调 service、把 `AppError` 映射为 `String`。不写业务，不碰数据库。
- **services**：业务规则。涉及"记录+回滚"统一走 `op_pipeline`；涉及"事件推送"统一走 `events`。
- **db**：每个表/表组一个 repo；连接通过 `Connection::open` 按需打开（无连接池）。"记录+items" 模式统一走 `op_record_repo`。

---

## 5. 核心抽象：op_record / op_pipeline / OpsPanel

后缀修改、空文件夹清理、Mod 重命名 / 归类 / 重复删除 / 旧版本删除 / 修改版本限制，本质都是："产生一批 `(old_path, new_path)` → 批量执行 → 写入可回滚记录"。项目把这一模式抽成三层，**任何新增此类业务都必须走这套抽象，严禁复制 suffix / mod_tools 当模板**。

### 5.1 后端

**`db::op_record_repo`**：通用 CRUD，通过 `OpRecordTables` 描述符传入表名与附加列（如 `target_suffix` / `kind`）。导出：
- `create_record` / `batch_insert_items` / `list_records(filter_extra_eq)` / `get_record_detail`
- `batch_update_rollback_results` / `set_record_rollback_status` / `delete_record` / `rename_record`

中性返回类型：`OpRecordSummary { record_id, record_name, extra, created_at, rollback_status, total_items, success_items }` / `OpRecordItem` / `OpRecordDetail`。**item 表结构硬约束**：`id / record_id / old_path / new_path / apply_success / apply_error / rollback_success / rollback_error / updated_at`——业务侧新增字段要另建辅助表。

**`services::op_pipeline`**：
- `resolve_thread_count(db_path)`：从用户设置读有效线程数（0 → num_cpus）。**所有并发操作都走这里，严禁内联读 `settings.thread_count`**。
- `resolve_io_concurrency_multiplier(db_path)`：IO 并发倍率（默认 2，范围 1–16）。dedup 与 Mod 扫描的 Semaphore 许可 = `线程数 × 倍率`。SSD/NVMe 可上调到 4–8，HDD 压到 1。
- `resolve_text_preview_max_bytes(db_path)` / `resolve_zip_preview_max_entries(db_path)`：预览上限。
- `parallel_move(pairs, create_parent, thread_count)`：并行 `std::fs::rename`；`create_parent = true` 自动建目标父目录（归类需要）。
- `parallel_execute<F>(pairs, thread_count, executor)`：并行执行自定义闭包（如 modify 的"备份+重写 zip"）。
- `persist_apply_rename_pairs(...)`：纯 rename 一步完成"建记录 + 并行 rename + 写 item"。
- `persist_apply_with_executor(..., executor)`：执行器自定义。
- `check_rollback(...)` / `rollback(..., force_ignore_missing)`：统一撤回实现。

**撤回约定**：item 表记 `old_path` 与 `new_path`，默认 rollback = `rename(new_path → old_path)`。modify 业务把 `old_path = 原文件, new_path = 备份` 即可复用——apply 改写原文件、备份旁边放一份；rollback 自动把备份覆盖回原文件。空文件夹清理写 `old_path = new_path = 目录路径`，撤回由业务侧自定义为 `create_dir_all(old_path)`。

### 5.2 前端

**`types/opRecord.ts`** 提供 `OpApplyItem` / `OpApplyResponse` / `OpRecordSummary<Extra>` / `OpRecordItem` / `OpRecordDetail<Extra>` / `OpRollbackCheck` / `OpRollbackResponse`。如 `SuffixRecordSummary = OpRecordSummary<{ targetSuffix }>`、`ModOpRecordSummary = OpRecordSummary<{ kind: ... }>`。

**`components/common/OpsPanel.vue`** 泛型面板，props：
- `paths` / `ensureNormalizedPaths`：路径规范化
- `columns` / `rows` / `rowKey`：VirtualTable 配置
- `preview` / `apply` / `checkRollback` / `rollback`：四个回调
- `applyItems` / `lastRecordId`：用于"撤回选中"的 itemId 映射
- `applyConfirmText` / `applyButtonText` / `previewToastBuilder` / `applySelectionFilter` / `infoTip`：UI 定制
- `#topForm` 插槽放业务专属表单（如"目标后缀"输入）

`SuffixPanel` / `ModRenamePanel` / `ModOrganizePanel` / `EmptyDirsPanel` 都是 **< 100 行** 的薄包装，只定义列 + rows computed + 四个回调。

**`ModScanPanel` 不走 OpsPanel**——"扫描"是长任务（绑定 `task_id` 事件），但其"修改选中"按钮调 `store.applyModifyVersion`，最终仍走 `op_pipeline::persist_apply_with_executor` 写入 `mod_op_records` 表。

### 5.3 新增同类业务的 checklist（5 处改动）

以"文件复制记录"为例：

1. **DB schema**：`schema.rs` 加 `CREATE TABLE IF NOT EXISTS copy_op_records / copy_op_items`，item 表对齐 op_record_repo 硬约束。
2. **service**：`services::copy_tools` 定义 `const COPY_TABLES: OpRecordTables = ...`；apply 调 `persist_apply_rename_pairs`（或 `persist_apply_with_executor`）；记录管理调 `op_record_repo`。
3. **command**：`commands::copy_tools`，在 `lib.rs::invoke_handler!` 注册。
4. **前端 types/service/store**：`OpRecordSummary<Extra>` 特化；service 封装命令；store 参考 `suffix.ts` / `modTools.ts`。
5. **UI**：挂 `<OpsPanel>` 薄包装；记录管理页加 tab 或 `kind` 过滤。

---

## 6. IPC 契约

### 事件清单

| 事件名 | 载荷 | 用途 |
|--------|------|------|
| `task_log` | `TaskLogPayload` | 实时日志 |
| `task_progress` | `TaskProgressPayload` | 进度更新 |
| `task_state_changed` | `{ taskId, status }` | 状态变更 |
| `task_failed` | `{ taskId, message }` | 失败通知 |
| `task_result_partial` | `{ taskId, groups, done }` | 去重增量结果 |
| `task_completed` | `{ taskId, groups }` | 去重完成 |
| `move_report_ready` | `{ taskId, report, updatedGroups }` | 移动完成 |
| `mod_scan_completed` | `ModScanCompletedPayload` | Mod 扫描完成 |
| `mod_duplicate_partial` | `ModDuplicatePartialPayload` | 重复 MOD 增量结果 |
| `mod_version_partial` | `ModVersionPartialPayload` | 不同版本 MOD 增量结果 |

事件名字面量统一在 [src-tauri/src/constants.rs](src-tauri/src/constants.rs) `events` 模块与前端 service 内，禁止硬编码。

### 命令清单（按 `lib.rs::invoke_handler!` 注册顺序）

- **路径**：`normalize_input_paths` / `reveal_in_explorer`
- **去重 / 运行时 / 移动**：`start_dedup_task` / `pause_task` / `resume_task` / `stop_task` / `get_move_summary` / `apply_move_action`
- **空文件夹清理**：`preview_empty_dirs` / `apply_empty_dir_cleanup` / `list_empty_dir_records` / `get_empty_dir_record_detail` / `check_empty_dir_rollback` / `rollback_empty_dir_cleanup` / `delete_empty_dir_record`
- **预览**：`request_preview`
- **设置 / 数据库**：`get_settings` / `save_settings` / `set_theme_mode` / `get_db_info` / `set_custom_db_path` / `delete_database` / `get_cpu_count`
- **哈希记录**：`list_hash_records` / `load_hash_record` / `rename_hash_record` / `delete_hash_record`
- **后缀修改**：`preview_suffix_change` / `apply_suffix_change` / `list_suffix_change_records` / `get_suffix_change_record_detail` / `check_suffix_rollback` / `delete_suffix_change_record` / `rollback_suffix_change`
- **Mod 工具**：`preview_mod_rename` / `apply_mod_rename` / `preview_mod_organize` / `apply_mod_organize` / `preview_mod_duplicates` / `start_mod_duplicate_task` / `apply_mod_duplicate_delete` / `preview_mod_versions` / `start_mod_version_task` / `apply_mod_version_delete` / `apply_mod_modify_version` / `list_mod_op_records` / `get_mod_op_record_detail` / `check_mod_op_rollback` / `rollback_mod_op` / `delete_mod_op_record` / `rename_mod_op_record` / `start_mod_scan_task`

### 长任务 task_id 约定

`start_dedup_task` 与 Mod 各类长任务（`start_mod_scan_task` / `start_mod_duplicate_task` / `start_mod_version_task`）都接受可选 `taskId`——前端预生成 ID 并先开始监听，避免事件早于监听器到达造成丢失。Mod 同步命令（重命名 / 归类 / modify）也接受可选 `taskId` 用来发 `task_log`。所有长任务在终态（成功 / 失败 / 取消）后会从 `AppState.tasks` 移除自身，避免 HashMap 单调增长。

### 重复 / 版本检查的扫描流程

只走一次 WalkDir：先收集候选 PathBuf 列表，用 `len()` 作为进度 total；第二阶段分块（chunk = 256）并行解析 manifest，每个 chunk 处理完通过 `mod_duplicate_partial` / `mod_version_partial` 增量下发，避免一次性大响应。

新增命令的步骤：写 `commands/<mod>.rs` → `lib.rs::invoke_handler!` 注册 → 前端 `services/<feature>.ts` 封装 → `types/<feature>.ts` 同步类型。

---

## 7. 代码风格规范

### 通用

- 前后端共享结构体必须 `#[serde(rename_all = "camelCase")]`，前端 TS 接口对齐驼峰。
- 事件 / 状态 / 枚举字符串统一放 `constants.rs` / `constants/`，禁止硬编码。
- 不在运行态调用 `println!` / `eprintln!` / `console.log`；日志走 `events::emit_log`。`bootstrap` 之前的启动期错误可以 `eprintln!`（前端尚未连接）。

### Rust

- **模块注释**：每个 `.rs` 顶部 `//!` 说明模块职责。
- **函数/类型注释**：所有 `pub fn` / `pub struct` / `pub enum` 都要 `///`。涉及"并发、长路径、事务、事件顺序、取消语义"的写明 WHY 与调用约定。
- **错误**：内部函数返 `AppResult<T>`；命令层把 `AppError` `.to_string()`。`rusqlite::Error` / `std::io::Error` / `serde_json::Error` 都有 `From` 实现，直接 `?` 传播。
- **路径**：`std::fs` 调用前一律 `to_extended_length_path`；展示或写库一律 `to_user_friendly_path`。
- **文件名工具**：`split_name_ext` / `resolve_conflict` / `strip_conflict_suffix` / `normalize_suffix` / `extract_bracket` 等都在 [utils/filename.rs](src-tauri/src/utils/filename.rs)，禁止业务模块内联同名函数。
- **并发**：线程数统一 `op_pipeline::resolve_thread_count(db_path)`；自定义线程池用 `rayon::ThreadPoolBuilder::build().install()` 本地化，不碰全局池。
- **数据库迁移**：新增表用 `CREATE TABLE IF NOT EXISTS`；新增列 `let _ = conn.execute("ALTER TABLE ... ADD COLUMN ...")` 忽略重复列错误。不改已有列。
- **记录型操作**：直接用 `op_record_repo` + `op_pipeline`，非纯 rename 用 `persist_apply_with_executor`。

### TypeScript / Vue

- **顶部 JSDoc**：每个 `.ts` / `.vue` 顶部 `/** ... */` 说明职责。service 指向后端命令，store 说明状态域，组件说明 props。
- **导出函数**：每个 `export function` 至少一行 JSDoc；复杂 composable / store action 解释 WHY。
- **类型**：业务代码禁止 `any`（OpsPanel 内部通用回调可接 `unknown` + 子类型）。`OpRecordSummary<Extra>` 通过 intersection 扩业务字段。
- **Vue**：统一 `<script setup lang="ts">`。表单控件用 Element Plus（`el-button` / `el-input` / `el-select` …），结构容器用**自有原语**——**不再使用 `el-card` / `el-tabs` / `el-scrollbar`**。长列表一律 `VirtualTable`。"预览→应用→撤回"类面板一律 `OpsPanel` 薄包装。
- **持久化**：跨会话用 `useStorage`；跨组件用 Pinia。
- **IPC**：组件内不直接 `invoke` / `listen`，走 `services/<feature>.ts`。
- **对话框**：确认用 `ElMessageBox.confirm`；输入用 `ElMessageBox.prompt`；不用原生 `window.confirm` / `window.prompt`。
- **TabBar 的 v-model**：Vue 编译器不支持 `v-model="x as any"`，泛值联合类型用显式写法：
  ```vue
  <TabBar :model-value="activeTab" :items="tabs"
          @update:model-value="(v: string) => (activeTab = v as 'a' | 'b')" />
  ```

### 注释原则（WHY > WHAT）

- **写 WHY**：非直觉约束、容易踩坑的边界、并发顺序、调用契约。
- **不写 WHAT**：好的命名本身就是文档。
- 示例：
  - ✅ `// 已提交的哈希任务不检查暂停状态，让它跑完`
  - ✅ `// bootstrap 阶段尚未初始化日志系统，只能直接打印到 stderr`
  - ✅ `// 记录 old=原文件, new=备份：默认 rollback 就能把备份覆盖回去`
  - ❌ `// 创建一个 HashMap` / `// 遍历 paths`

---

## 8. 组件设计约定

### 全局布局根 & 设计令牌

- `src/styles/index.css`：
  - **设计令牌**：`--ff-space-*` / `--ff-radius-*` / `--ff-font-*` / `--ff-bg-*` / `--ff-text-*` / `--ff-border-*` / `--ff-shadow-*`。业务 CSS 只从这里挑值，禁止裸数值（`13px` / `0 2px 4px rgba(...)`）。
  - **根重置**：`html / body / #app` 锁视口高度 + 取消默认外边距；`body { overflow: hidden; overscroll-behavior: none }`。否则 body 默认 8px 外边距叠 100vh 子元素会让文档高出视口，html 层冒出多余滚动条。
  - **暗色主题**：`html.dark` 覆盖全部令牌；业务 CSS 不 hard-code 颜色，`useTheme` 切 `html.dark` 即可整站换肤。
  - **`.ff-scroll`**：业务滚动容器统一的低调滚动条（含 `scrollbar-gutter: stable`）。
- `.app-shell` (App.vue) 用 grid 布局侧栏 + 分隔条 + 主内容；`height: 100vh; overflow: hidden`。`.app-viewport` 自身 `overflow: hidden`——**页面路由不做外层滚动**，每个页面 root 决定溢出行为。

### Panel —— 替代 el-card

`el-card__header / __body` 是写死的内部类名；想让 body 变 flex column 让 VirtualTable auto-height 生效，只能 `:deep(.el-card__body)` 覆写，每次 EP 升级可能崩，嵌套 tabs / drawer 时 :deep 链越拉越长。Panel 自控结构：`.panel-header`（可选）+ `.panel-body (flex: 1; min-height: 0; flex column)` + `.panel-footer`（可选）。

- `padded` 默认 true：body 带 padding + gap，适合表单。子节点是 VirtualTable 时传 `padded=false` 贴边。
- `compact`：header 高度从 44px 压到 36px。
- `flat`：去背景/边框/阴影，用于嵌套场景。
- header 支持 `#header` 与 `#actions` 两个 slot。

### TabBar —— 替代 el-tabs

`el-tabs__content` 不是 flex column，要 `:deep()` 强改；更糟的是切 pane 时 `display: none / block` 切换会搅动 flex item 初始高度，触发 VirtualTable 的 ResizeObserver 反复 fire——肉眼上是滚动条反复闪烁。

TabBar 只渲染按钮条，内容切换交给调用方：**用 `v-show` 而非 `v-if`**，配合 `position: absolute; inset: 0` 的 `.tab-host` 容器，所有子面板铺满同一区域，切 tab 时零 reflow。

```vue
<div class="tab-host">
  <FooPanel v-show="tab === 'foo'" />
  <BarPanel v-show="tab === 'bar'" />
</div>
<style>
.tab-host { flex: 1; min-height: 0; position: relative; }
.tab-host > * { position: absolute; inset: 0; }
</style>
```

视觉为段式控件（Segmented Control），选中项高亮为 panel 底色 + 轻阴影。`size="small"` 用于嵌套子 tab。

### VirtualTable

固定行高虚拟滚动；支持列拖拽、全选、客户端/服务端分页、固定列（sticky）、ellipsis + tooltip、空态占位、**列自定义（显示/左固定/顺序）**、双击复制单元格文本。列定义类型 `VirtualColumn`，运行时配置类型 `VirtualColumnState`。列宽拖拽 `requestAnimationFrame` 节流。

**滚动条不闪烁的关键：`scrollbar-gutter: stable`**。`.vtable-scroll` 永远预留 ~15px 纵向滚动条宽度，让 `clientWidth` 在内容行变多/变少时不再震荡；否则 fit-width 模式下纵向条出现 → clientWidth 缩小 → 列宽重算 → 内容收住 → 纵向条消失 → clientWidth 复位 → 反复闪烁。fit-width 模式下还强制 `overflow-x: hidden` 兜底 1px 舍入；ResizeObserver 回调 rAF 合并多次 entry。

**高度两种模式**：
- 固定高度：传 `:height="<px>"`。仅当父容器无固定高度又必须确定尺寸时使用；目前只有 `MoveReportDialog`（el-dialog 按内容撑高）。
- 自适应：省略 `height`，`.vtable` 与 `.vtable-scroll` 都用 `flex: 1; min-height: 0`。**主页面 / 详情抽屉 / OpsPanel / ModScanPanel 统一此模式**。硬要求：**父链路径一路 `display: flex; flex-direction: column; min-height: 0`**——任何一层断 flex 或少 `min-height: 0` 都会塌缩或溢出多余滚动条。不要写 `tableHeight = panelHeight - 180` 这种魔数。`el-drawer__body` 等需要 `:deep()` 改成 flex column，参见 `RecordManagePage.vue` / `RecordDetailDrawer.vue`。

**列宽自适应（`fit-width`）**：开启后总列宽永远等于容器宽度，消除横向滚动。算法用 `width - minWidth` 作为伸缩权重：只声明 `width` 视为固定列（权重 0），只声明 `minWidth` 是弹性列。所有列 `minWidth` 之和大于容器宽时回退横滚。OpsPanel / ModScanPanel 默认开启；不传 `fit-width` 则保持原行为（如 RecordManagePage 主表）。

**虚拟滚动不走 VueUse `useVirtualList`**——它自带 `overflow-y: auto` 容器 + `marginTop` 偏移 wrapper，会与"单一滚动容器"打架，是历史上"滚动条与位置不同步 / 固定列只有首行跟滚"的根因。手写实现：监听 `.vtable-scroll.scroll`（rAF 节流）→ 按 `scrollTop / itemHeight` 算可见区间 → 可见行 `position: absolute; top: N*itemHeight` 放进 `height = totalRows*itemHeight` 的占位 `.body`。`rows.length` 减少时若 `scrollTop` 超新范围会自动回滚到 `maxTop`。

**关键结构约束**（踩坑产物，别改）：
- 最外层 `.vtable { display: flex; flex-direction: column; width: 100%; min-width: 0; overflow: hidden }`——撑满父容器，不用 `fit-content`，避免列总宽 < 容器时右侧露父容器底色。
- `.vtable-toolbar`（`columnConfigurable=true` 时出现，默认 true）放 `.vtable-scroll` 之上，右对齐"列设置"按钮。不参与滚动。
- **只有一个滚动容器**：`.vtable-scroll { overflow: auto }` 同时承担横纵滚动。这样三件事同时被解决：
  - 滚动条位置和实际内容始终同步（嵌套滚动会出现"到底了滑块没到底"）
  - 所有 body 行的 `position: sticky; left: X` 共享同一 scroll ancestor，横滚时固定列对齐视口左边（嵌套时 sticky 找内层纵滚容器，它不横滚 → 固定列只首行生效）
  - 每个表格只有一条横滚条与一条纵滚条
- `.vtable-content { width: totalColumnWidth; min-width: 100% }`：宽度锚定列总宽，`min-width: 100%` 兜底撑满滚动容器。
- `.head` / `.body` / `.row` 不显式写宽，统一 `width: 100%` 跟随 `.vtable-content`。列宽拖拽抖动由 `.vtable-content` 显式 px 宽度压住（不是 `max-content`）。
- 空态在 `.body` 内渲染，`.body` 高度回退 `bodyViewHeight`（滚动容器减 36px 表头），empty 图居中。
- `getRowKey` 找不到稳定键时退到 `__idx:{n}`，**绝不**用 `Math.random()`（破坏 v-for 复用与选择状态）。
- sticky 固定列：head `z-index: 4`，body `z-index: 2`；背景必须显式不透明（head 用 `--el-fill-color-light`，body 用 `--el-bg-color`）。

**列自定义约定**：
- selection 列（`__select__`）不在自定义范畴：永远第一列、永远左固定、不能隐藏、不能移动。
- 其余列的显示 / 左固定 / 顺序在"列设置"弹层里调（`el-popover` + pointer 事件拖拽）。仅开放左固定。**不用 HTML5 DnD**——Tauri WebView 会吞掉 dragstart（保留外部文件拖入通道），表现为光标变"禁止"、drop 不生效。pointer 事件是底层 DOM 事件 Tauri 不拦截，drop target 用 `document.elementFromPoint` + `.closest('.col-config-row')` 定位。
- **固定列必须是紧跟 selection 的连续左前缀**：
  - `setFixed(key, true)`：左侧所有可见列一并设为固定（前缀闭合）
  - `setFixed(key, false)`：该列及右侧所有可见列一并解除（后缀解除）
  - 拖拽重排后 `normalizeFixedContiguity` 兜底：扫到非固定列后，其后所有固定列强制解除。隐藏列不参与约束。
- 传 `column-config-key="<stable-id>"` 持久化到 localStorage（`vtable:col:<stable-id>`，VueUse `useStorage`），不传则会话内生效。命名建议 `feature:table-name`（如 `records:hash-list`）。
- `columns` 运行时变更自动 reconcile：新增列追加到末尾、被删除列剔除，再 `normalizeFixedContiguity`。
- 关闭功能退回旧版表头：传 `:column-configurable="false"`。

### OpsPanel

"预览 → 应用 → 撤回本次 / 撤回选中"统一交互。顶部支持 `#topForm` 插槽放业务专属控件。父组件传 4 个回调和列定义即可。内部负责：VirtualTable 渲染、勾选管理、按钮状态、极限数据量提示、缺失路径确认对话框。

OpsPanel / ModScanPanel 走 `.ops-panel / .main-card / VirtualTable` 一路 flex column 撑满（VirtualTable auto-height）。**修改这些面板时不要给 VirtualTable 传 `height`，不要回到 `Math.max(260, panelHeight - 180)` 魔数**。

### SettingsPage 导航

左侧 180px 固定导航（sticky 在 grid 列）+ 右侧 `.settings-scroll` 独立滚动。点击导航 `el.scrollIntoView({ behavior: "smooth", block: "start" })` 定位；滚动时 `IntersectionObserver` 自动高亮当前可视分组，`rootMargin: "-40% 0px -60% 0px"` 把判定区域贴近用户视线中部，避免分组过渡时高亮闪烁。

### RealtimeLogPanel

手写虚拟滚动，行高 30px。日志更新后 `scrollTop = scrollHeight` 自动跟随；滚到底部 ≤5px 自动开启跟随，离开底部 >50px 自动关闭。

### DuplicateGroupTable

`el-collapse` 手风琴；分段加载（`renderLimits`）处理大分组；全部勾选警告可关闭。**只在 `.group-container` 这一层开纵向滚动**——内部 `el-table` 不限高、外层不套 `el-scrollbar`，避免"外层 + 中层 + 表内"三条滚动条叠加。

### 路径行操作（hover-to-clickable）

Mod 各面板（Rename / Organize / Duplicate / Version / Scan）的"原文件路径"列：通过 VirtualTable 列的 `slotName` 暴露 slot，包一层 `<PreviewPanel :path="row.xxx">` 提供 hover 预览，内部放 `<button class="path-link" @click="revealInExplorer(path)">` 提供点击跳转。`.path-link` 是 block / 100% 宽 / 支持 ellipsis，hover 下划线，disabled 静默。**不再单独保留"目录"操作列**。

---

## 9. 性能与并发约定

- **高频日志事件**：每条 `task_log` 是单条 IPC。前端 `runtime` store 把事件先写非响应式缓冲，每 150ms 批量刷入响应式 `logs`，超过 `LOG_MAX_LENGTH = 3000` 裁剪旧数据。后端如需进一步合批要新增 `task_log_batch` 事件并改前端监听器（目前未做）；热路径请只在 chunk / 阶段边界打日志。`PARTIAL_BATCH_SIZE = 30` 仅用于去重 `task_result_partial`。
- **去重流水线**：扫描 → 哈希（Semaphore 限流，许可 = 线程数 × 倍率）→ 分组 → 发送。流式 `mpsc` 通道边收边分组。
- **Mod 扫描流水线**：tokio Semaphore 并发 = `线程数 × 倍率`；zip 读取 `tokio::task::spawn_blocking`；匹配结果进 `Arc<Mutex<Vec<_>>>`。
- **重复 / 不同版本检查流水线**：单次 WalkDir 长任务。第一遍只收候选 PathBuf 列表（用 `len()` 当 total），第二阶段固定 chunk = 256 用本地 rayon 池并行解析 manifest，每 chunk 完成立刻聚合 + 增量推送（`mod_duplicate_partial` / `mod_version_partial`），避免一次性大响应。
- **modify 流水线**：rayon 并行；每个文件 copy → 重写 zip（`raw_copy_file` 零重编码复制非 manifest 条目）→ atomic rename。失败自动清理临时文件 + 备份。
- **暂停/取消**：`TaskRuntime::{paused, cancelled}` 为 `AtomicBool`；扫描阶段只响应取消，哈希调度阶段同时响应取消与暂停；已提交的哈希任务跑完。
- **Windows 长路径**：内部全部加 `\\?\` 前缀（`to_extended_length_path`），对外去前缀（`to_user_friendly_path` / `stripWindowsExtendedPrefix`）。

---

## 10. 主题、路由、持久化

- **主题**：`light` / `dark` / `system`；`useTheme` 基于 VueUse `useDark` + `usePreferredDark`，`html.dark` class 切换 EP 暗色。持久化写 localStorage + `app_settings.theme_mode`。
- **路由**：Hash 模式三条：`/`（任务中心）、`/settings`、`/records`。
- **外部配置**：数据库路径存 `<app_data_dir>/kk-file-tool_config.json`（鸡生蛋问题——无法把 db 路径存进 db 本身）。后端 `external_config::resolve_db_path` 已做"是目录就追加 kk-file-tool.db"的兜底，前端选目录后直接传即可。

---

## 11. 修改代码时的注意事项

1. **前后端模型同步**：改 `models/*.rs` 必须同步改 `types/*.ts`，字段驼峰对齐。
2. **命令注册**：新 `#[tauri::command]` 必须在 `lib.rs::invoke_handler!` 注册并写前端 service。
3. **记录型操作**：走 `op_record_repo + op_pipeline + OpsPanel`，不要复制 suffix / mod_tools 当模板。非纯 rename 用 `persist_apply_with_executor`，item 记 `old_path = 原始, new_path = 备份`。
4. **数据库迁移**：只允许 `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE ... ADD COLUMN`，不改已有列。迁移写在 `schema.rs::init_schema` 末尾。
5. **IPC 事件**：新事件登记 `constants.rs::events`，写 `services/events.rs` emit 函数，在前端 store 的 `initEvents` 监听。
6. **路径**：`std::fs` 入参一律 `to_extended_length_path`；返回前端的路径一律 `to_user_friendly_path`。
7. **并发**：线程数从 `op_pipeline::resolve_thread_count` 取，不要 inline 读 `settings.thread_count`。
8. **文件名**：用 `utils::filename` 里的函数，不要自己实现一份。
9. **注释**：新的 `pub fn` / `pub struct` / `pub enum` 必须 `///`；新的 TS `export function` / store action 必须 JSDoc。
10. **验证**：`cargo check`（src-tauri/）+ `npx vue-tsc --noEmit`（根目录）双把关；UI 流程跑 `npm run tauri dev`。
11. **同步本文档**：按 §0 把本轮改动反映到对应段落。

---

## 附录：常量速查

### 事件 / 枚举（硬编码，不开放给用户）

- 事件：`constants::events::{TASK_LOG, TASK_PROGRESS, TASK_STATE_CHANGED, TASK_FAILED, TASK_RESULT_PARTIAL, TASK_COMPLETED, MOVE_REPORT_READY, MOD_SCAN_COMPLETED, MOD_DUPLICATE_PARTIAL, MOD_VERSION_PARTIAL}`
- 阶段：`constants::stages::{SCAN, HASH, MOD_SCAN, MOD_DUPLICATE, MOD_VERSION}`
- 日志等级：`constants::log_level::{INFO, WARN, ERROR}`
- 保留策略：`constants::keep_policy::{NEWEST, OLDEST}`
- 主题：`constants::theme::{LIGHT, DARK, SYSTEM}`
- Mod 操作类型：`constants::mod_op_kind::{RENAME, ORGANIZE, MODIFY, DUPLICATE_DELETE, VERSION_DELETE}`
- 空文件夹操作类型：`constants::empty_dir_op_kind::DELETE`
- 哈希状态：`constants::hash_entry_status::ACTIVE`
- 数据库文件：`constants::db_file::{DEFAULT_NAME, WAL_EXT, SHM_EXT}`

### 配置项（存 `app_settings` 表，"配置中心"页可调，自动保存）

| 字段 | 类型 | 默认 | 影响 |
|------|------|------|------|
| `keep_policy` | str | `newest` | 去重 / 重复 MOD / 不同版本默认保留策略 |
| `move_target_path` | str? | null | 重复文件移动目标目录 |
| `save_record_enabled` | bool | true | 哈希索引是否入库 |
| `use_last_record_enabled` | bool | false | 去重时是否复用上次哈希记录 |
| `include_current_folder_duplicates` | bool | true | 是否统计当前目录内重复 |
| `theme_mode` | str | `system` | `light` / `dark` / `system` |
| `thread_count` | i32 | 0 | 并发核心数；0 = num_cpus |
| `log_max_length` | i32 | 3000 | 前端日志保留条数 |
| `io_concurrency_multiplier` | i32 | 2 | IO 并发倍率（×有效线程数） |
| `extreme_row_threshold` | i32 | 20000 | 虚拟表极限模式阈值 |
| `text_preview_max_kb` | i32 | 256 | 文本预览最大字节（KiB） |
| `zip_preview_max_entries` | i32 | 5000 | 压缩包预览枚举上限 |
| `mod_scan_default_keyword` | str | `Koikatsu` | Mod 扫描关键字默认 |
| `suffix_default_target` | str | `txt` | 后缀修改默认目标（不带点） |

新增配置项的步骤：`models/settings.rs` 加字段 + 默认 → `db/schema.rs` 末尾 `ALTER TABLE ADD COLUMN` → `db/settings_repo.rs` 的 SELECT/UPDATE 扩列 → `types/settings.ts` 加字段 → `stores/config.ts` 初始 state 加默认 → `views/SettingsPage.vue` 加表单项。`DEFAULT_*` 兜底常量集中在 [src-tauri/src/config.rs](src-tauri/src/config.rs)。

### 非配置常量（编译期硬编码）

- 前端：`src/constants/app.ts`（`DEFAULT_LOG_MAX_LENGTH` 兜底、`LOG_FLUSH_INTERVAL`）、`task.ts`（`DEFAULT_EXTREME_ROW_THRESHOLD` 兜底 / `EXTREME_OVERSCAN` / `NORMAL_OVERSCAN` / 分组分页与渲染步长）、`theme.ts`、`preview.ts`。
- 后端：`src-tauri/src/config.rs`（`HASH_QUEUE_SIZE` / `SCAN_QUEUE_SIZE` / `PARTIAL_BATCH_SIZE` / `PAUSE_SLEEP_MS`，加一组 `DEFAULT_*` 作为配置兜底）。
