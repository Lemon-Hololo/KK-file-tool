# FileFlow Desktop — 项目规范

本文档是 FileFlow Desktop 的架构与风格规范，也是 AI 辅助开发时的 single source of truth。修改代码前先读，修改代码后必须同步更新。

---

## 0. 文档维护

- 本文档描述的是**当前代码的真实状态**，不是路线图。任何不符合现状的描述都是 bug，应立即修正。
- 任何会话在做以下改动时，**必须同步更新本文档**：
  - 新增 / 删除 / 重命名模块、数据库表、Tauri 命令、事件、常量；
  - 改动公共抽象（`op_pipeline` / `op_record_repo` / `OpsPanel` / `VirtualTable` 等）的 API、调用契约或结构约束；
  - 改动并发模型、路径处理、注释规范等跨模块约定；
  - 调整目录结构或分层语义。
- 更新文档时**以当前状态为准直接覆写**对应段落，不要留"已删除 / 已废弃 / 新增"之类的补丁语气——读者只关心现在是什么样，不关心历史。

---

## 1. 项目概述

FileFlow Desktop 是一个基于 Tauri 2 的 Windows 桌面文件处理平台，主要功能：

- **文件去重**：BLAKE3 哈希识别重复文件，支持暂停/恢复/停止，支持增量移动。
- **后缀批量修改**：批量修改扩展名，支持预览 / 应用 / 撤回 / 历史记录。
- **空文件夹清理**：递归预览空目录，确认后按深度从深到浅删除并写入可撤回记录；撤回会重新创建空目录，默认不删除任务输入根目录。
- **Mod 工具**：针对 Illusion 系列 `.zipmod` 文件的六类操作——
  - **重命名**：按 manifest.xml 的 `guid/author/version` 生成 `[author] guid-version.zipmod`；同批次命中同名目标时按稳定顺序自动分配 ` (N)` 冲突后缀，避免相互覆盖。
  - **归类**：按文件名首个 `[...]` 建子目录并归类。
  - **重复 MOD 检查**：按 `guid + author + version` 分组找重复文件，默认每组保留修改时间最新文件，可切换保留最旧或保留指定文件；删除移动到 `.fileflow-del-*` 备份路径并可撤回。
  - **不同版本 MOD 检查**：按 `guid + author` 分组找多个 `version`，默认保留最高版本，可保留指定文件；删除移动到 `.fileflow-del-*` 备份路径并可撤回。
  - **移除版本限制（modify）**：对选中的 zipmod 就地重写 manifest.xml 去掉指定 `<game>KEYWORD</game>` 标签，原文件备份以便撤回。
  - **版本限制扫描**：长任务，扫描结果勾选后直接交给"移除版本限制"落盘为 Mod 操作记录。
- **文件预览**：文本 / 图片 / 压缩包内容查看；压缩包预览列出内部路径、大小、目录标记与条目修改时间。
- **记录管理**：哈希记录、后缀记录、空文件夹清理记录、Mod 操作记录统一在"记录管理"页管理；Mod 记录按 `kind` 分 `rename` / `organize` / `modify` / `duplicate_delete` / `version_delete`。

---

## 2. 技术栈

### 前端

| 技术 | 版本 | 用途 |
|------|------|------|
| Vue 3 | ^3.5 | Composition API + `<script setup>` |
| TypeScript | ^5.9 | 严格模式 |
| Pinia | ^3.0 | Options API 风格 store |
| Element Plus | ^2.13 | UI 组件库 |
| VueUse | ^14.2 | 组合式工具（useStorage、useDark、useElementSize、useVirtualList 等） |
| Vue Router | ^5.0 | Hash 模式 |
| Vite | ^7.3 | 构建工具 |
| @tauri-apps/api | ^2.10 | IPC |
| @tauri-apps/plugin-dialog | ^2.6 | 原生对话框 |

### 后端（Rust）

| 技术 | 版本 | 用途 |
|------|------|------|
| Tauri | 2.10 | 桌面框架(protocol-asset) |
| tokio | 1.49 | 异步运行时(rt-multi-thread, macros, sync) |
| rusqlite | 0.38 | SQLite（bundled + WAL） |
| blake3 | 1.8 | 文件哈希 |
| walkdir | 2.5 | 递归遍历 |
| rayon | 1.10 | 并行迭代；本项目用局部 `ThreadPoolBuilder::install` 避免污染全局池 |
| quick-xml | 0.36 | zipmod manifest.xml 解析 |
| encoding_rs | 0.8 | manifest 编码回退（UTF-8 / GBK / Shift_JIS） |
| zip | 8.1 | 压缩包读取、`raw_copy_file` 零重编码重写 |
| image | 0.25 | 图片元信息预览 |
| uuid | 1.21 | UUID v4 |
| chrono | 0.4 | 时间处理 |
| num_cpus | 1.17 | CPU 核心数 |
| serde / serde_json | 1 | 序列化，统一 `rename_all = "camelCase"` |
| thiserror | 2.0 | `AppError` 派生 |

---

## 3. 目录结构

```
fileflow-desktop/
├── src/                                 # 前端
│   ├── App.vue / main.ts
│   ├── router/{index,routes}.ts
│   ├── views/
│   │   ├── TaskPage.vue                 # 任务中心（路径+日志 / 去重 / 后缀 / 空文件夹 / Mod 工具）
│   │   ├── SettingsPage.vue
│   │   └── RecordManagePage.vue         # 四类记录管理（Mod 记录按 kind 过滤）
│   ├── components/
│   │   ├── DedupPanel.vue               # 去重面板（独立，不走 OpsPanel）
│   │   ├── SuffixPanel.vue              # 薄包装 → OpsPanel
│   │   ├── EmptyDirsPanel.vue           # 空文件夹清理面板（薄包装 → OpsPanel）
│   │   ├── ModRenamePanel.vue           # 薄包装 → OpsPanel
│   │   ├── ModOrganizePanel.vue         # 薄包装 → OpsPanel
│   │   ├── ModDuplicatePanel.vue        # 重复 MOD 分组 + 删除（写 Mod 操作记录）
│   │   ├── ModVersionPanel.vue          # 不同版本 MOD 分组 + 删除（写 Mod 操作记录）
│   │   ├── ModScanPanel.vue             # 扫描 + 勾选 + 修改版本限制（modify）
│   │   ├── ModToolsPanel.vue            # 五个 Mod 子 Tab 容器（TabBar + v-show）
│   │   ├── RealtimeLogPanel.vue         # 手写虚拟滚动日志
│   │   ├── DuplicateGroupTable.vue
│   │   ├── PreviewPanel.vue / MoveConfirmDialog.vue / MoveReportDialog.vue
│   │   ├── RecordDetailDrawer.vue / TaskControlPanel.vue
│   │   └── common/
│   │       ├── Panel.vue                # 自有卡片原语，替代 el-card
│   │       ├── TabBar.vue               # 自有段式切换，替代 el-tabs
│   │       ├── VirtualTable.vue         # 通用虚拟表（head+body 共享横滚、支持空态）
│   │       └── OpsPanel.vue             # 通用"预览→应用→撤回"面板
│   ├── stores/                          # Pinia Options-style
│   │   ├── runtime.ts / task.ts / record.ts / config.ts / preview.ts
│   │   ├── suffix.ts / emptyDirs.ts / modTools.ts
│   ├── services/                        # IPC 封装，对应后端命令
│   │   ├── tauri.ts                     # invokeCmd / onEvent 基础
│   │   ├── task.ts / settings.ts / record.ts / preview.ts
│   │   ├── suffix.ts / emptyDirs.ts / modTools.ts
│   ├── types/
│   │   ├── common.ts / task.ts / settings.ts / record.ts / moveReport.ts / preview.ts / virtualTable.ts
│   │   ├── opRecord.ts                  # 通用可撤回操作记录类型
│   │   ├── suffix.ts / modTools.ts      # 基于 opRecord 特化
│   │   ├── emptyDirs.ts                 # 空文件夹清理类型
│   ├── composables/
│   │   ├── useTheme.ts                  # 主题切换
│   │   └── usePathNormalize.ts          # 路径规范化 + 警告弹窗
│   ├── utils/
│   │   ├── path.ts                      # uniquePaths / stripWindowsExtendedPrefix
│   │   ├── format.ts / error.ts
│   │   └── mapper.ts                    # dedup 分组前端包装透传
│   └── constants/
│       ├── app.ts / task.ts / theme.ts / preview.ts
│
├── src-tauri/src/                       # 后端
│   ├── main.rs / lib.rs                 # 入口 + 命令注册
│   ├── app_state.rs                     # AppState + TaskRuntime
│   ├── constants.rs                     # 事件名 / 阶段 / 枚举字符串集中
│   ├── config.rs                        # 编译期常量
│   ├── error.rs                         # AppError + AppResult
│   ├── external_config.rs               # 启动前可读的 JSON 配置（数据库路径）
│   ├── models/                          # 按领域拆分的 DTO，全部 camelCase serde
│   │   ├── mod.rs / task.rs / hash_record.rs / move_file.rs / path_norm.rs
│   │   ├── settings.rs / suffix.rs / empty_dirs.rs / mod_tools.rs
│   ├── db/                              # 数据访问层
│   │   ├── mod.rs / schema.rs
│   │   ├── op_record_repo.rs            # 通用"操作记录"仓储（suffix / mod_op 共用）
│   │   ├── hash_repo.rs / move_repo.rs / settings_repo.rs
│   ├── services/                        # 业务逻辑层
│   │   ├── mod.rs / events.rs / logging.rs
│   │   ├── op_pipeline.rs               # 通用 preview→apply→rollback 流水线
│   │   ├── suffix.rs / empty_dirs.rs
│   │   ├── mod_tools/
│   │   │   ├── mod.rs                   # 记录查询/删除/重命名/撤回（映射到 op_pipeline）
│   │   │   ├── rename.rs / organize.rs  # 纯 rename：构造 pairs → op_pipeline::persist_apply_rename_pairs
│   │   │   ├── cleanup.rs               # 重复 / 不同版本检查；分块并行读取 manifest，删除移动到可回滚备份路径
│   │   │   ├── modify.rs                # 非纯 rename：备份 + 重写 zip → op_pipeline::persist_apply_with_executor
│   │   │   ├── scan.rs / zipmod.rs
│   │   ├── dedup.rs / move_file.rs / preview.rs
│   ├── commands/                        # #[tauri::command]，纯转发
│   │   ├── mod.rs / path.rs / dedup.rs / runtime.rs / move_file.rs
│   │   ├── preview.rs / settings.rs / records.rs
│   │   ├── suffix.rs / empty_dirs.rs / mod_tools.rs
│   └── utils/
│       ├── mod.rs / path.rs / hash.rs / fs.rs
│       └── filename.rs                  # split_name_ext / resolve_conflict / strip_conflict_suffix /
│                                        # normalize_suffix / extract_bracket / sanitize_filename / normalize_brackets
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

- **commands 层**：只解析参数、调 service、把 `AppError` 映射为 `String`。不写业务，不碰数据库。
- **services 层**：业务规则。涉及"记录+回滚"的操作统一走 `services::op_pipeline`；涉及"事件推送"的操作统一走 `services::events`。
- **db 层**：每个表/表组一个 repo；连接统一通过 `Connection::open` 按需打开（无连接池）。通用的"记录+items"模式统一用 `db::op_record_repo`。

---

## 5. 核心抽象：op_record / op_pipeline / OpsPanel

后缀修改、空文件夹清理、Mod 重命名 / 归类 / 重复删除 / 旧版本删除 / 修改版本限制的本质都是："产生一批 `(old_path, new_path)` → 批量执行 → 写入可回滚记录"。FileFlow 把这一模式抽成三个层次，**任何新增的此类业务都必须走这套抽象，严禁复制 suffix / mod_tools 作为模板**。

### 5.1 后端

**`db::op_record_repo`**（[src-tauri/src/db/op_record_repo.rs](src-tauri/src/db/op_record_repo.rs)）：通用 CRUD。通过 `OpRecordTables` 描述符传入表名与附加列（`target_suffix` / `kind`）。导出函数：
- `create_record` / `batch_insert_items` / `list_records(filter_extra_eq)` / `get_record_detail`
- `batch_update_rollback_results` / `set_record_rollback_status` / `delete_record` / `rename_record`

中性返回类型：`OpRecordSummary { record_id, record_name, extra, created_at, rollback_status, total_items, success_items }` / `OpRecordItem` / `OpRecordDetail`。item 表结构硬约束：`id / record_id / old_path / new_path / apply_success / apply_error / rollback_success / rollback_error / updated_at`——业务侧新增字段要另建辅助表。

**`services::op_pipeline`**（[src-tauri/src/services/op_pipeline.rs](src-tauri/src/services/op_pipeline.rs)）：
- `resolve_thread_count(db_path)`：从用户设置读取有效线程数（0 → num_cpus）。**所有并发操作都要调这个函数，严禁再次内联读 `settings.thread_count`**。
- `resolve_io_concurrency_multiplier(db_path)`：读用户设置里的 IO 并发倍率（默认 2，范围 1–16），dedup 与 Mod 扫描的 Semaphore 许可 = `resolve_thread_count × 本倍率`。SSD/NVMe 用户可在设置中心调到 4–8，HDD 压到 1。
- `resolve_text_preview_max_bytes(db_path)` / `resolve_zip_preview_max_entries(db_path)`：文本与压缩包预览的上限；保存在设置里，前端可调。
- `parallel_move(pairs, create_parent, thread_count)`：并行 `std::fs::rename`；`create_parent = true` 自动创建目标父目录（归类需要）。
- `parallel_execute<F>(pairs, thread_count, executor)`：并行执行自定义闭包（不是纯 rename 的场景用，如 modify 的"备份 + 重写 zip"）。
- `persist_apply_rename_pairs(db, tables, extra, name, source_paths, pairs, create_parent)`：纯 rename 一步完成"建记录 + 并行 rename + 写 item"。
- `persist_apply_with_executor(..., executor)`：同上，但执行器自定义。
- `check_rollback(db, tables, record_id, item_ids)` / `rollback(db, tables, record_id, item_ids, force_ignore_missing)`：统一的撤回实现。

**撤回约定**：item 表记 `old_path` 与 `new_path`，默认 rollback = `rename(new_path → old_path)`。modify 业务把 `old_path = 原文件路径, new_path = 备份路径` 即可复用这套撤回——apply 时把原文件改写、备份旁边放一份；rollback 时自动把备份覆盖回原文件。空文件夹清理写 `old_path = new_path = 目录路径`，撤回由业务侧自定义为 `create_dir_all(old_path)`。

### 5.2 前端

**`types/opRecord.ts`**：
```ts
OpApplyItem / OpApplyResponse
OpRecordSummary<Extra>      // Extra 承载业务字段
OpRecordItem / OpRecordDetail<Extra>
OpRollbackCheck / OpRollbackResponse
```
`SuffixRecordSummary = OpRecordSummary<{ targetSuffix }>`，`ModOpRecordSummary = OpRecordSummary<{ kind: "rename"|"organize"|"modify" }>`。

**`components/common/OpsPanel.vue`**：泛型面板，props 接收：
- `paths / ensureNormalizedPaths`：路径规范化
- `columns / rows / rowKey`：VirtualTable 配置
- `preview / apply / checkRollback / rollback`：四个回调
- `applyItems / lastRecordId`：用于"撤回选中"的 itemId 映射
- `applyConfirmText / applyButtonText / previewToastBuilder / applySelectionFilter / infoTip`：可选 UI 定制
- `#topForm` 插槽放业务专属表单项（如"目标后缀"输入）

三个面板 [SuffixPanel.vue](src/components/SuffixPanel.vue) / [ModRenamePanel.vue](src/components/ModRenamePanel.vue) / [ModOrganizePanel.vue](src/components/ModOrganizePanel.vue) 都是 **< 80 行** 的薄包装，只定义列 + rows computed + 四个回调。

**[ModScanPanel.vue](src/components/ModScanPanel.vue) 不走 OpsPanel**，因为"扫描"是长任务（通过 `task_id` 绑定事件），但其"修改选中"按钮仍调 `store.applyModifyVersion`，后端最终走 `op_pipeline::persist_apply_with_executor` 写入同一套 `mod_op_records` 表。

### 5.3 新增一种同类业务的 checklist（仅 5 处改动）

假设要加"文件复制记录"：

1. **DB schema**：在 `schema.rs` 加 `CREATE TABLE IF NOT EXISTS copy_op_records / copy_op_items`。item 表结构必须对齐 op_record_repo 硬约束。
2. **service**：新建 `services::copy_tools`，定义 `const COPY_TABLES: OpRecordTables = ...`；apply 直接调 `op_pipeline::persist_apply_rename_pairs`（或 `persist_apply_with_executor`）；记录管理调 `op_record_repo`。
3. **command**：`commands::copy_tools`，在 `lib.rs::invoke_handler!` 注册。
4. **前端 types/service/store**：`types/copyTools.ts` 用 `OpRecordSummary<Extra>` 特化；`services/copyTools.ts` 封装命令；store 参考 `suffix.ts` / `modTools.ts` 写法。
5. **UI**：挂 `<OpsPanel>` 薄包装；记录管理页加一个 tab 或 `kind` 过滤即可。

---

## 6. IPC 契约

### 事件清单

| 事件名 | 方向 | 载荷 | 用途 |
|--------|------|------|------|
| `task_log` | 后→前 | `TaskLogPayload` | 实时日志 |
| `task_progress` | 后→前 | `TaskProgressPayload` | 进度更新 |
| `task_state_changed` | 后→前 | `{ taskId, status }` | 状态变更 |
| `task_failed` | 后→前 | `{ taskId, message }` | 失败通知 |
| `task_result_partial` | 后→前 | `{ taskId, groups, done }` | 去重增量结果 |
| `task_completed` | 后→前 | `{ taskId, groups }` | 去重完成 |
| `move_report_ready` | 后→前 | `{ taskId, report, updatedGroups }` | 移动完成 |
| `mod_scan_completed` | 后→前 | `ModScanCompletedPayload` | Mod 扫描完成 |
| `mod_duplicate_partial` | 后→前 | `ModDuplicatePartialPayload` | 重复 MOD 增量结果 |
| `mod_version_partial` | 后→前 | `ModVersionPartialPayload` | 不同版本 MOD 增量结果 |

事件名字面量统一在 [src-tauri/src/constants.rs](src-tauri/src/constants.rs) `events` 与前端 service 内，不要硬编码。

### 命令清单（按领域，按 `lib.rs::invoke_handler!` 注册顺序）

- **路径**：`normalize_input_paths` / `reveal_in_explorer`
- **去重 / 运行时 / 移动**：`start_dedup_task` / `pause_task` / `resume_task` / `stop_task` / `get_move_summary` / `apply_move_action`
- **空文件夹清理**：`preview_empty_dirs` / `apply_empty_dir_cleanup` / `list_empty_dir_records` / `get_empty_dir_record_detail` / `check_empty_dir_rollback` / `rollback_empty_dir_cleanup` / `delete_empty_dir_record`
- **预览**：`request_preview`
- **设置 / 数据库**：`get_settings` / `save_settings` / `set_theme_mode` / `get_db_info` / `set_custom_db_path` / `delete_database` / `get_cpu_count`
- **哈希记录**：`list_hash_records` / `load_hash_record` / `rename_hash_record` / `delete_hash_record`
- **后缀修改**：`preview_suffix_change` / `apply_suffix_change` / `list_suffix_change_records` / `get_suffix_change_record_detail` / `check_suffix_rollback` / `rollback_suffix_change` / `delete_suffix_change_record`
- **Mod 工具**：`preview_mod_rename` / `apply_mod_rename` / `preview_mod_organize` / `apply_mod_organize` / `preview_mod_duplicates` / `start_mod_duplicate_task` / `apply_mod_duplicate_delete` / `preview_mod_versions` / `start_mod_version_task` / `apply_mod_version_delete` / `apply_mod_modify_version` / `list_mod_op_records` / `get_mod_op_record_detail` / `check_mod_op_rollback` / `rollback_mod_op` / `delete_mod_op_record` / `rename_mod_op_record` / `start_mod_scan_task`

扫描结果通过勾选 + `apply_mod_modify_version` 直接落盘为 Mod 操作记录，不导出 TXT。
Mod 的同步命令（重命名 / 归类 / modify）允许前端传 `taskId`，后端据此发 `task_log`；`start_mod_scan_task` / `start_mod_duplicate_task` / `start_mod_version_task` 也支持前端显式传入 `taskId`。重复 MOD 与不同版本检查现在按长任务运行：日志与进度实时推送，结果通过 `mod_duplicate_partial` / `mod_version_partial` 增量下发，避免一次性巨大响应。

新增命令：在 `commands/<mod>.rs` 写命令 → 在 `lib.rs` `invoke_handler!` 注册 → 前端 `services/<feature>.ts` 封装 → `types/<feature>.ts` 同步类型。

---

## 7. 代码风格规范

### 通用

- 所有前后端共享结构体必须 `#[serde(rename_all = "camelCase")]`，对应前端 TS 接口驼峰。
- 事件 / 状态 / 枚举等字符串常量统一放入 `constants.rs` / `constants/`，禁止硬编码。
- 不在运行态调用 `println!` / `eprintln!` / `console.log`；日志走 `events::emit_log`。`app_state::bootstrap` 之前的启动期错误可以 `eprintln!`（此时前端尚未连接）。

### Rust

- **模块注释**：每个 `.rs` 顶部写 `//!` 说明模块职责；IPC 契约或并发约定放在模块级。
- **函数/类型注释**：所有 `pub fn` / `pub struct` / `pub enum` 都要 `///` 文档注释。短函数一行说明即可；涉及"并发、长路径、事务、事件顺序、取消语义"的函数要写明 WHY 与调用约定。
- **错误**：内部函数返回 `AppResult<T>`；命令层在 `Result<T, String>` 边界上把 `AppError` `.to_string()`。`rusqlite::Error` / `std::io::Error` / `serde_json::Error` 都有 `From` 实现，直接 `?` 传播即可。
- **路径**：所有 `std::fs` 调用前一律 `utils::path::to_extended_length_path`；展示或写库一律 `to_user_friendly_path`。
- **文件名工具**：`split_name_ext` / `resolve_conflict` / `strip_conflict_suffix` / `normalize_suffix` / `extract_bracket` 等都在 [utils/filename.rs](src-tauri/src/utils/filename.rs)，不要再在业务模块里内联同名函数。
- **并发**：线程数统一 `op_pipeline::resolve_thread_count(db_path)`；需要自定义线程池时用 `rayon::ThreadPoolBuilder::build().install()` 本地化，不碰全局池。
- **数据库迁移**：新增表用 `CREATE TABLE IF NOT EXISTS`；新增列用 `let _ = conn.execute("ALTER TABLE ... ADD COLUMN ...")` 忽略重复列错误。不要改已有列。
- **记录型操作**：严禁复制 suffix / mod_tools 作为模板，直接用 `op_record_repo` + `op_pipeline`。非纯 rename 用 `persist_apply_with_executor`。

### TypeScript / Vue

- **顶部 JSDoc**：每个 `.ts` / `.vue` 顶部一段 `/** ... */` 说明职责：service 指向对应后端命令，store 说明管理的状态域，组件说明 props 语义。
- **导出函数**：每个 `export function` 至少一行 JSDoc。复杂 composable / store action 解释 WHY（如日志批量缓冲、虚拟滚动边界条件）。
- **类型**：业务代码禁止 `any`（OpsPanel 内部的通用行回调可以接受 `unknown` + 具体子类型）；服务返回都有泛型类型。`OpRecordSummary<Extra>` 通过 intersection 扩展业务字段。
- **Vue**：统一 `<script setup lang="ts">`；表单控件用 Element Plus（`el-button` / `el-input` / `el-select` / …），结构类容器用**自有原语**——**不再使用 `el-card` / `el-tabs` / `el-scrollbar`**。长列表一律 `VirtualTable`。"预览 → 应用 → 撤回"类面板一律通过 `common/OpsPanel.vue` 薄包装，不写自己的按钮组与确认对话框。
- **持久化**：跨会话状态用 `useStorage`；跨组件状态用 Pinia。
- **IPC**：组件内不要直接 `invoke` / `listen`，走 `services/<feature>.ts` 封装。
- **对话框**：确认用 `ElMessageBox.confirm`；输入用 `ElMessageBox.prompt`；不要用原生 `window.confirm` / `window.prompt`。
- **TabBar 的 v-model**：Vue 编译器不支持 `v-model="x as any"`，给泛值联合类型做 v-model 要用显式写法：
  ```vue
  <TabBar :model-value="activeTab" :items="tabs"
          @update:model-value="(v: string) => (activeTab = v as 'a' | 'b')" />
  ```

### 注释原则（WHY > WHAT）

- **写 WHY**：非直觉的约束、容易踩坑的边界、并发顺序、历史债务、调用契约。
- **不写 WHAT**：不要解释显而易见的代码。好的命名本身就是文档。
- **示例**：
  - ✅ `// 已提交的哈希任务不检查暂停状态，让它跑完`
  - ✅ `// bootstrap 阶段尚未初始化日志系统，只能直接打印到 stderr`
  - ✅ `// 记录 old=原文件, new=备份：默认 rollback 就能把备份覆盖回去`
  - ❌ `// 创建一个 HashMap` / `// 遍历 paths`

---

## 8. 组件设计约定

### 全局布局根 & 设计令牌

- `src/styles/index.css` 包含：
  - **设计令牌**：`--ff-space-*` 间距，`--ff-radius-*` 圆角，`--ff-font-*` 字号，`--ff-bg-*` / `--ff-text-*` / `--ff-border-*` 色板，`--ff-shadow-*` 阴影。所有业务 CSS 只从这里挑值，不再写 `13px` / `18px` / `0 2px 4px rgba(...)` 这种裸数值。
  - **根重置**：`html / body / #app` 都锁到视口高度 + 取消默认外边距；`body { overflow: hidden; overscroll-behavior: none }`。body 默认 8px 外边距叠上 `height: 100vh` 的子元素会让文档高出视口 → html 层冒出一条没人要的外层滚动条。
  - **暗色主题**：`html.dark` 下覆盖全部令牌；业务 CSS 不再 hard-code 颜色，`useTheme` 切换 `html.dark` 即可整站换肤。
  - **.ff-scroll 辅助类**：给业务滚动容器统一的低调滚动条（含 `scrollbar-gutter: stable`）。
- `.app-shell` (App.vue) 用 grid 把侧栏 + 分隔条 + 主内容布局好，`height: 100vh; overflow: hidden`。`.app-main` 内部 grid 分 `topbar / viewport` 两行，`.app-viewport` 本身 `overflow: hidden`——**页面路由不做外层滚动**，每个页面 root 自己决定溢出行为。

### Panel（[components/common/Panel.vue](src/components/common/Panel.vue)）—— 替代 el-card

**为什么不用 el-card**：`el-card__header / __body` 是写死的内部类名，想让 body 变 flex column 让 VirtualTable auto-height 生效只能 `:deep(.el-card__body) { ... }` 覆写，每次 EP 升级都可能崩，嵌套 tabs / drawer 时 :deep 链越拉越长。Panel 由我们自己控制结构，直接就是 flex column：`.panel-header`（可选）+ `.panel-body (flex: 1; min-height: 0; flex column)` + `.panel-footer`（可选）。

- `padded` 默认 true：body 带 padding + gap，适合表单 + 控件排列；子节点是 VirtualTable 时传 `padded=false` 贴边。
- `compact` 让 header 高度从 44px 压到 36px。
- `flat` 去掉背景/边框/阴影，用于场景嵌套（例如 Panel 套在另一个卡片里）。
- header 支持 `#header` 和 `#actions` 两个 slot，左侧标题右侧操作按钮，不再像 el-card 那样只能 flex 堆一行。

### TabBar（[components/common/TabBar.vue](src/components/common/TabBar.vue)）—— 替代 el-tabs

**为什么不用 el-tabs**：`el-tabs__content` 不是 flex column，要通过 `:deep()` 改成 `flex: 1; min-height: 0; overflow: hidden`，再把 tab-pane 也改成 flex column。更要命的是 el-tabs 切 pane 时 `display: none / block` 切换会搅动 flex item 的初始高度，触发 VirtualTable 的 ResizeObserver 反复 fire，**肉眼上就是滚动条反复闪烁**。

TabBar 只渲染按钮条，内容切换交给调用方：**用 `v-show` 而不是 `v-if`**，配合 `position: absolute; inset: 0` 的 `.tab-host` 容器，所有子面板铺满同一块区域，切 tab 时零 reflow，VirtualTable 的 ResizeObserver 一次都不会被触发。

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

TabBar 视觉为段式控件（Segmented Control），选中项高亮为 panel 底色 + 轻阴影。`size="small"` 用于嵌套子 tab（如 Mod 工具五个子页）。

### VirtualTable（[components/common/VirtualTable.vue](src/components/common/VirtualTable.vue)）

固定行高虚拟滚动；支持列拖拽、全选、客户端/服务端分页、固定列（sticky）、ellipsis + tooltip、空态占位、**列自定义（显示/左固定/顺序）**、双击复制单元格文本。列定义类型 `VirtualColumn`，用户运行时配置类型 `VirtualColumnState`。列宽拖拽用 `requestAnimationFrame` 节流。

**滚动条不闪烁的关键：`scrollbar-gutter: stable`**。`.vtable-scroll` 永远预留 ~15px 纵向滚动条宽度，让 `clientWidth` 在内容行变多 / 变少时不再震荡。原来 fit-width 模式下一旦内容行变多触发纵向滚动条出现，clientWidth 瞬间缩小 ~15px，列宽重算 → 内容刚好收住 → 纵向滚动条消失 → clientWidth 复位 → 列宽复位 → 纵向再出现…无限循环，肉眼就是"横/纵滚动条不停闪"。另外 fit-width 模式下强制 `overflow-x: hidden` 兜底 1px 舍入误差；ResizeObserver 回调用 rAF 合并多次 entry，避免同一帧多次 set 触发反复重算。

**高度两种模式**：
- 固定高度：传 `:height="<px>"`（仅当父容器本身没有固定高度、又必须让表格以确定尺寸出现时使用；目前只有 `MoveReportDialog` 这一处——`el-dialog` 按内容撑高），`.vtable-scroll` 用 inline height。
- 自适应：省略 `height`，`.vtable` 与 `.vtable-scroll` 都用 `flex: 1; min-height: 0` 撑满父容器。**主页面（TaskPage / RecordManagePage / 三类详情抽屉）与 OpsPanel / ModScanPanel 统一用此模式**。硬性要求：**父链路径一路 `display: flex; flex-direction: column; min-height: 0`**——任何一层断 flex 或少 `min-height: 0` 都会导致 flex item 无法正确分配剩余空间，表现为塌缩或溢出到 tab-pane / drawer 外出现多余滚动条。不要再写 `tableHeight = panelHeight - 180` 这种魔数去挤 px 值。`el-drawer__body` / `el-card__body` / `el-tabs__content` 都要用 `:deep()` 改成 flex column，参见 `RecordManagePage.vue` / `RecordDetailDrawer.vue` 的 `.detail-drawer :deep(.el-drawer__body)` 段。

**列宽自适应（`fit-width`）**：开启后总列宽永远等于容器宽度，消除横向滚动。算法用 `width - minWidth` 作为伸缩权重：只声明 `width` 的列视为固定列（权重 0），只声明 `minWidth` 的列是弹性列（按权重吃掉 / 分配剩余空间）。所有列的 `minWidth` 之和仍大于容器宽度时只能横滚，此时回退到 `colWidths`（尊重 minWidth 的可读性下限）。OpsPanel / ModScanPanel 默认开启。想完全不压缩列宽的表格（如 RecordManagePage 主表）不传 `fit-width` 即保持原行为。

**虚拟滚动不走 VueUse `useVirtualList`**——它自带 `overflow-y: auto` 的容器 + `marginTop` 偏移 wrapper，会和"单一滚动容器"需求打架，也是历史上"滚动条和实际位置不同步 / 固定列只有首行跟滚"的根因。现在自己写：监听 `.vtable-scroll.scroll`（raf 节流）→ 按 `scrollTop / itemHeight` 算可见区间 → 可见行 `position: absolute; top: N*itemHeight` 放进 `height = totalRows*itemHeight` 的占位 `.body` 里。`props.rows.length` 减少时，若 `scrollTop` 超出新范围会自动回滚到 `maxTop`，避免滚到空白。

**关键结构约束**（布局踩过坑，别改）：
- 最外层 `.vtable { display: flex; flex-direction: column; width: 100%; min-width: 0; border; overflow: hidden }`——始终撑满父容器，不再用 `fit-content`，避免列总宽 < 容器时表格右侧露出一片父容器底色的空白。
- `.vtable-toolbar`（可选，`columnConfigurable=true` 时出现，默认 true）放在 `.vtable-scroll` 之上，右对齐一个"列设置"按钮。它不参与滚动容器，宽度跟随 `.vtable`。
- **只有一个滚动容器**：`.vtable-scroll { overflow: auto }` 同时承担横向和纵向滚动。这样三件事同时被解决：
  - 滚动条位置和实际内容始终同步（之前嵌套滚动会出现"到底了滑块没到底 / 从底部拉不动"）
  - 所有 body 行的固定列 `position: sticky; left: X` 共享同一个 scroll ancestor，横滚时每一行的固定列都正确对齐视口左边（之前嵌套结构里 sticky 找到的是内层纵滚容器，它不横滚，于是固定列全失效，只有表头跟滚，视觉上像"只有首行固定列生效"）
  - 每个表格只有一条横滚条和一条纵滚条，不再"内一条外一条"重叠
- `.vtable-content { width: totalColumnWidth; min-width: 100% }`：宽度锚定列总宽，同时用 `min-width: 100%` 兜底把它撑到至少等于滚动容器宽，这样列总宽 < 容器时内容层也能铺满。
- `.head` / `.body` / `.row` 不再显式写 `width: ${totalColumnWidth}px`，统一 `width: 100%` 跟随 `.vtable-content`；列总宽 < 容器时最后一列右侧留白自然用表头底色 / 行底色填充，不会露出外层颜色。列宽拖拽抖动由 `.vtable-content` 的显式 px 宽度压住（不是 `max-content`）。
- 空态（`pagedRows.length === 0`）渲染在 `.body` 内，`.body` 此时高度回退为 `bodyViewHeight`（滚动容器减去 36px 表头），empty 图居中。
- `getRowKey` 找不到稳定键时退到 `__idx:{n}`，**绝不**用 `Math.random()`（会破坏 v-for 复用与选择状态）。
- sticky 固定列：head 用 `z-index: 4`（压住 body 固定列），body 用 `z-index: 2`；head / body 固定列背景必须显式不透明（head 用 `--el-fill-color-light`，body 用 `--el-bg-color`），否则横滚时后面的文字会透上来。

**列自定义约定**：
- selection 列（`__select__`）不在自定义范畴：永远是第一列、永远左固定、不能隐藏、不能移动。
- 其余列的显示 / 左固定 / 顺序由用户在"列设置"弹层里调整（Element Plus `el-popover` + pointer 事件拖拽）。仅开放左固定。列顺序拖拽**不用 HTML5 DnD**：Tauri WebView 会吞掉 HTML5 dragstart（因为它要为外部文件拖入保留通道），表现为拖动时光标变"禁止"、drop 不生效。pointer 事件是底层 DOM 事件，Tauri 不拦截，drop target 用 `document.elementFromPoint` + `.closest('.col-config-row')` 定位。
- **固定列必须是紧跟 selection 的连续左前缀**。这一约束贯穿所有操作：
  - `setFixed(key, true)`：把该列左侧所有可见列一并设为固定（前缀闭合）
  - `setFixed(key, false)`：把该列及右侧所有可见列一并解除固定（后缀解除）
  - 拖拽重排后调用 `normalizeFixedContiguity` 兜底：一旦扫到非固定列，其后所有固定列强制解除。隐藏列不参与约束。
- 传 `column-config-key="<stable-id>"` 可持久化到 localStorage（`vtable:col:<stable-id>`），通过 VueUse `useStorage` 存储；不传则仅会话内生效。命名建议 `feature:table-name`（如 `records:hash-list`）。
- `columns` 运行时变更会自动 reconcile：新增列追加到末尾（默认显示、按原 `fixed` 决定固定状态），被删除的列从配置剔除，然后 `normalizeFixedContiguity` 清理残留。
- 需要关闭该功能退回旧版表头：传 `:column-configurable="false"`。

### OpsPanel（[components/common/OpsPanel.vue](src/components/common/OpsPanel.vue)）

"预览 → 应用 → 撤回本次 / 撤回选中"的统一交互。顶部支持 `#topForm` 插槽放业务专属控件。父组件只需传 4 个回调和列定义。内部负责：VirtualTable 渲染、勾选管理、按钮状态、极限数据量提示、缺失路径确认对话框。

OpsPanel 与 ModScanPanel 走 `.ops-panel / .main-card / .el-card__body → VirtualTable` 一路 flex column 撑满的布局（VirtualTable 用 auto-height）。**修改这些面板时不要给 VirtualTable 传 `height`，不要回到 `Math.max(260, panelHeight - 180)` 这种魔数**——算不准就会在 `el-tabs__content` 里冒出多余滚动条。

### RealtimeLogPanel

手写虚拟滚动，行高 30px；日志更新后 `scrollTop = scrollHeight` 实现自动跟随；滚到底部 ≤5px 自动开启跟随，离开底部 >50px 自动关闭。

### DuplicateGroupTable

`el-collapse` 手风琴；分段加载 (`renderLimits`) 处理大分组；全部勾选警告可关闭。**只在 `.group-container` 这一层开纵向滚动**（`overflow-y: auto`），内部 `el-table` 不限高、外层不再套 `el-scrollbar`——避免"外层 scrollbar + 中层 scrollbar + 表内 body-wrapper scrollbar"三条滚动条叠加。分段加载配合单滚动容器已经足够控制 DOM 规模。

---

## 9. 性能与并发约定

- **高频事件（日志）**：后端批量发送（`PARTIAL_BATCH_SIZE = 30`），前端 `runtime` store 把事件先写入非响应式缓冲，每 150ms 批量刷入响应式 `logs` 数组，超过 `LOG_MAX_LENGTH = 3000` 裁剪旧数据。
- **去重流水线**：扫描 → 哈希（Semaphore 限流，许可 = 有效线程数 × 2）→ 分组 → 发送。流式 `mpsc` 通道边收边分组。
- **Mod 扫描流水线**：tokio Semaphore 并发 = `resolve_thread_count × resolve_io_concurrency_multiplier`；每个文件的 zip 读取用 `tokio::task::spawn_blocking`；匹配结果进一个 `Arc<Mutex<Vec<_>>>`。
- **重复 / 不同版本检查流水线**：两段式长任务。第一轮 WalkDir 只统计候选 `.zip/.zipmod` 数量，第二轮按固定 chunk（当前 256）缓存候选路径，并用 `resolve_thread_count` 控制的本地 rayon 线程池并行读取 manifest；每个 chunk 处理完立刻聚合进分组 map，并通过 `mod_duplicate_partial` / `mod_version_partial` 增量推给前端，避免同时持有"全量候选路径 + 全量解析结果 + 一次性大响应"。
- **modify 流水线**：rayon 并行；每个文件 copy → 重写 zip（`raw_copy_file` 零重编码复制非 manifest 条目） → atomic rename。失败自动清理临时文件 + 备份。
- **暂停/取消**：`TaskRuntime::{paused, cancelled}` 为 `AtomicBool`；扫描阶段只响应取消，哈希调度阶段同时响应取消与暂停；已提交的哈希任务跑完。
- **Windows 长路径**：后端内部全部加 `\\?\` 前缀（`to_extended_length_path`），对外展示去前缀（`to_user_friendly_path` / `stripWindowsExtendedPrefix`）。

---

## 10. 主题、路由、持久化

- **主题**：三种模式 `light` / `dark` / `system`；`useTheme` composable 基于 VueUse `useDark` + `usePreferredDark`，通过 `html.dark` CSS class 切换 Element Plus 暗色。持久化写 localStorage + SQLite `app_settings.theme_mode`。
- **路由**：Hash 模式，三条：`/`（任务中心）、`/settings`、`/records`。
- **外部配置**：数据库路径存 `<app_data_dir>/fileflow_config.json`（鸡生蛋问题——无法把 db 路径存进 db 本身）。

---

## 11. 修改代码时的注意事项

1. **前后端模型同步**：改 `models/*.rs` 必须同步改 `types/*.ts`；字段驼峰对齐。
2. **命令注册**：新增 `#[tauri::command]` 必须在 `lib.rs::invoke_handler!` 注册，并在前端写 service 封装。
3. **记录型操作**：新增 preview→apply→rollback 模式的业务时，走 `op_record_repo + op_pipeline + OpsPanel`。不要复制 suffix / mod_tools 当模板。非纯 rename 的（修改文件内容等）用 `persist_apply_with_executor`，item 记 `old_path = 原始, new_path = 备份`，默认 rollback 即可恢复。
4. **数据库迁移**：只允许 `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE ... ADD COLUMN`，不改已有列。迁移写在 `schema.rs::init_schema` 末尾。
5. **IPC 事件**：新增事件要在 `constants.rs::events` 登记，在 `services/events.rs` 写 emit 函数，在前端对应 store 的 `initEvents` 里监听。
6. **路径**：一切 `std::fs` 入参都要经过 `to_extended_length_path`；返回给前端的路径都要经过 `to_user_friendly_path`。
7. **并发**：线程数从 `op_pipeline::resolve_thread_count` 取；不要 inline 读 `settings.thread_count`。
8. **文件名处理**：用 `utils::filename` 里的函数（`split_name_ext` / `resolve_conflict` / `strip_conflict_suffix` / `normalize_suffix` / `extract_bracket`），不要再自己实现一份。
9. **注释**：新增的 `pub fn` / `pub struct` / `pub enum` 必须带 `///`；新增的 TS `export function` / `defineStore` action 必须有 JSDoc。
10. **验证**：类型安全由 `cargo check`（src-tauri/）+ `npx vue-tsc --noEmit`（根目录）双把关；UI 流程跑 `npm run tauri dev` 人工验证。
11. **同步本文档**：按 §0 的规则，把本轮改动反映到 CLAUDE.md 的对应段落里。

---

## 附录：常量速查

### 事件 / 枚举（硬编码，不开放给用户）

- 事件：`constants::events::{TASK_LOG, TASK_PROGRESS, TASK_STATE_CHANGED, TASK_FAILED, TASK_RESULT_PARTIAL, TASK_COMPLETED, MOVE_REPORT_READY, MOD_SCAN_COMPLETED}`
- 阶段：`constants::stages::{SCAN, HASH, MOD_SCAN}`
- 日志等级：`constants::log_level::{INFO, WARN, ERROR}`
- 保留策略：`constants::keep_policy::{NEWEST, OLDEST}`
- 主题：`constants::theme::{LIGHT, DARK, SYSTEM}`
- Mod 操作类型：`constants::mod_op_kind::{RENAME, ORGANIZE, MODIFY, DUPLICATE_DELETE, VERSION_DELETE}`（`is_valid(v)` 供命令层校验）
- 空文件夹操作类型：`constants::empty_dir_op_kind::DELETE`
- 哈希状态：`constants::hash_entry_status::ACTIVE`
- 数据库文件：`constants::db_file::{DEFAULT_NAME, WAL_EXT, SHM_EXT}`

### 配置项（存在 `app_settings` 表，前端在"配置中心"页调；修改后自动保存）

| 字段 | 类型 | 默认 | 影响 |
|------|------|------|------|
| `keep_policy` | str | `newest` | 去重默认保留策略，同时作为重复 MOD / 不同版本检查的默认保留策略 |
| `move_target_path` | str? | null | 重复文件移动目标目录 |
| `save_record_enabled` | bool | true | 哈希索引是否入库 |
| `use_last_record_enabled` | bool | false | 去重时是否复用上次哈希记录 |
| `include_current_folder_duplicates` | bool | true | 是否统计当前目录内重复 |
| `theme_mode` | str | `system` | `light` / `dark` / `system` |
| `thread_count` | i32 | 0 | 并发核心数；0 = num_cpus |
| `log_max_length` | i32 | 3000 | 前端日志保留条数 |
| `io_concurrency_multiplier` | i32 | 2 | IO 并发倍率（×有效线程数） |
| `extreme_row_threshold` | i32 | 20000 | 虚拟表进入极限模式的行数阈值 |
| `text_preview_max_kb` | i32 | 256 | 文本预览最大字节（KiB） |
| `zip_preview_max_entries` | i32 | 5000 | 压缩包预览枚举上限 |
| `mod_scan_default_keyword` | str | `Koikatsu` | Mod 扫描关键字默认 |
| `suffix_default_target` | str | `txt` | 后缀修改默认目标（不带点） |

新增配置项的步骤：`models/settings.rs` 加字段 + 默认 → `db/schema.rs` 末尾 `ALTER TABLE ADD COLUMN` → `db/settings_repo.rs` 的 SELECT/UPDATE 扩列 → `types/settings.ts` 加字段 → `stores/config.ts` 初始 state 加默认 → `views/SettingsPage.vue` 加表单项。所有 `DEFAULT_*` 常量集中在 [src-tauri/src/config.rs](src-tauri/src/config.rs)；UI fallback 常量放 `src/constants/app.ts` 或 `src/constants/task.ts`。

### 非配置常量（编译期硬编码）

前端：`src/constants/app.ts`（`DEFAULT_LOG_MAX_LENGTH` 兜底、`LOG_FLUSH_INTERVAL`）、`task.ts`（`DEFAULT_EXTREME_ROW_THRESHOLD` 兜底 / `EXTREME_OVERSCAN` / `NORMAL_OVERSCAN` / 分组分页与渲染步长）、`theme.ts`。后端：`src-tauri/src/config.rs`（`HASH_QUEUE_SIZE` / `SCAN_QUEUE_SIZE` / `PARTIAL_BATCH_SIZE` / `PAUSE_SLEEP_MS`，加上一组 `DEFAULT_*` 作为配置兜底）。
