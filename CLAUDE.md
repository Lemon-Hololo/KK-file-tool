# FileFlow Desktop — 项目规范

> 本文档是 FileFlow Desktop 项目的完整架构与风格规范。AI 辅助开发时请把此文档作为系统提示/上下文加载，代码修改必须遵守其中的分层、命名、并发、注释约定。

---

## 1. 项目概述

FileFlow Desktop 是一个基于 Tauri 2 的 Windows 桌面文件处理平台，主要功能：

- **文件去重**：BLAKE3 哈希识别重复文件，支持暂停/恢复/停止，支持增量移动。
- **后缀批量修改**：批量修改扩展名，支持预览 / 应用 / 撤回 / 历史记录。
- **Mod 工具**：针对 Illusion 系列 `.zipmod` 文件的重命名、按作者归类、关键字扫描。
- **文件预览**：文本 / 图片 / 压缩包内容查看。
- **记录管理**：哈希记录、后缀记录、Mod 操作记录统一在"记录管理"页管理。

---

## 2. 技术栈

### 前端

| 技术 | 版本 | 用途 |
|------|------|------|
| Vue 3 | ^3.5 | Composition API + `<script setup>` |
| TypeScript | ^5.9 | 严格模式 |
| Pinia | ^3.0 | Options API 风格 store |
| Element Plus | ^2.13 | UI 组件库 |
| VueUse | ^14.2 | 组合式工具（useStorage、useDark、useElementSize 等） |
| Vue Router | ^5.0 | Hash 模式 |
| Vite | ^7.3 | 构建工具 |
| @tauri-apps/api | ^2.10 | IPC |
| @tauri-apps/plugin-dialog | ^2.6 | 原生对话框 |

### 后端（Rust）

| 技术 | 版本 | 用途 |
|------|------|------|
| Tauri | 2.10 | 桌面框架（protocol-asset） |
| tokio | 1.49 | 异步运行时（rt-multi-thread, macros, sync） |
| rusqlite | 0.38 | SQLite（bundled + WAL） |
| blake3 | 1.8 | 文件哈希 |
| walkdir | 2.5 | 递归遍历 |
| rayon | 1.10 | 并行迭代；本项目用局部 `ThreadPoolBuilder::install` 避免污染全局池 |
| quick-xml | 0.36 | zipmod manifest.xml 解析 |
| zip | 8.1 | 压缩包读取 |
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
│   │   ├── TaskPage.vue                 # 任务中心（路径+日志 / 去重 / 后缀 / Mod 工具）
│   │   ├── SettingsPage.vue
│   │   └── RecordManagePage.vue         # 三类记录管理
│   ├── components/
│   │   ├── DedupPanel.vue               # 去重面板
│   │   ├── SuffixPanel.vue              # 薄包装 → OpsPanel
│   │   ├── ModRenamePanel.vue           # 薄包装 → OpsPanel
│   │   ├── ModOrganizePanel.vue         # 薄包装 → OpsPanel
│   │   ├── ModScanPanel.vue             # 独立（扫描长任务）
│   │   ├── ModToolsPanel.vue            # 三个 Mod 子 Tab 容器
│   │   ├── RealtimeLogPanel.vue         # 手写虚拟滚动日志
│   │   ├── DuplicateGroupTable.vue
│   │   ├── PreviewPanel.vue / MoveConfirmDialog.vue / MoveReportDialog.vue
│   │   ├── RecordDetailDrawer.vue / TaskControlPanel.vue
│   │   └── common/
│   │       ├── VirtualTable.vue         # 通用虚拟表
│   │       └── OpsPanel.vue             # 通用"预览→应用→撤回"面板
│   ├── stores/                          # Pinia Options-style
│   │   ├── runtime.ts / task.ts / record.ts / config.ts / preview.ts
│   │   ├── suffix.ts / modTools.ts
│   ├── services/                        # IPC 封装，对应后端命令
│   │   ├── tauri.ts                     # invokeCmd / onEvent 基础
│   │   ├── task.ts / settings.ts / record.ts / preview.ts
│   │   ├── suffix.ts / modTools.ts
│   ├── types/
│   │   ├── common.ts / task.ts / settings.ts / record.ts / moveReport.ts / preview.ts / virtualTable.ts
│   │   ├── opRecord.ts                  # 通用可撤回操作记录类型
│   │   ├── suffix.ts / modTools.ts      # 基于 opRecord 特化
│   ├── composables/
│   │   ├── useTheme.ts                  # 主题切换
│   │   └── usePathNormalize.ts          # 路径规范化 + 警告弹窗
│   ├── utils/
│   │   ├── path.ts                      # uniquePaths / stripWindowsExtendedPrefix
│   │   ├── format.ts / mapper.ts / error.ts
│   └── constants/
│       ├── app.ts / task.ts / theme.ts / preview.ts
│
├── src-tauri/src/                       # 后端
│   ├── main.rs / lib.rs                 # 入口 + 命令注册
│   ├── app_state.rs                     # AppState + TaskRuntime
│   ├── constants.rs                     # 事件名/阶段/枚举字符串集中
│   ├── config.rs                        # 编译期常量
│   ├── error.rs                         # AppError + AppResult
│   ├── external_config.rs               # 启动前可读的 JSON 配置（数据库路径）
│   ├── models/                          # 按领域拆分的 DTO，全部 camelCase serde
│   │   ├── mod.rs / task.rs / hash_record.rs / move_file.rs / path_norm.rs
│   │   ├── settings.rs / suffix.rs / mod_tools.rs
│   ├── db/                              # 数据访问层
│   │   ├── mod.rs / schema.rs
│   │   ├── op_record_repo.rs            # 通用"操作记录"仓储（suffix / mod_op 共用）
│   │   ├── hash_repo.rs / move_repo.rs / settings_repo.rs
│   ├── services/                        # 业务逻辑层
│   │   ├── mod.rs / events.rs
│   │   ├── op_pipeline.rs               # 通用 preview→apply→rollback 流水线
│   │   ├── suffix.rs
│   │   ├── mod_tools/
│   │   │   ├── mod.rs                   # 记录查询/删除/重命名/撤回（映射到 op_pipeline）
│   │   │   ├── rename.rs / organize.rs  # 构造 pairs，调 op_pipeline
│   │   │   ├── scan.rs / zipmod.rs
│   │   ├── dedup.rs / move_file.rs / preview.rs
│   ├── commands/                        # #[tauri::command]，纯转发
│   │   ├── mod.rs / path.rs / dedup.rs / runtime.rs / move_file.rs
│   │   ├── preview.rs / settings.rs / records.rs
│   │   ├── suffix.rs / mod_tools.rs
│   └── utils/
│       ├── mod.rs / path.rs / hash.rs / fs.rs
│       └── filename.rs                  # split_name_ext / resolve_conflict / extract_bracket ...
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

## 5. 共享抽象：op_record / op_pipeline / OpsPanel

后缀修改、Mod 重命名、Mod 归类三类业务的本质都是："产生一批 `(old_path, new_path)` → 批量 rename → 写入可回滚记录"。FileFlow 把这一模式抽成三个层次：

### 后端

1. **`db::op_record_repo`**：通用 CRUD。通过 `OpRecordTables` 描述符传入表名与附加列（`target_suffix` / `kind`）。函数：`create_record` / `batch_insert_items` / `list_records` / `get_record_detail` / `batch_update_rollback_results` / `set_record_rollback_status` / `delete_record` / `rename_record`。
2. **`services::op_pipeline`**：
   - `resolve_thread_count(db_path)`：从用户设置读取有效线程数（0 → num_cpus）；**所有并发操作都要调这个函数，不要再内联重写**。
   - `parallel_move(pairs, create_parent, thread_count)`：并行 `std::fs::rename`；`create_parent = true` 会自动创建目标父目录（归类需要）。
   - `persist_apply_rename_pairs(db, tables, extra_value, name, source_paths, pairs, create_parent)`：一步完成"创建记录 + 并行 rename + 写 item"。
   - `check_rollback(db, tables, record_id, item_ids)` / `rollback(db, tables, record_id, item_ids, force_ignore_missing)`：统一的撤回实现。

`services::suffix` 与 `services::mod_tools::{rename, organize}` 都是薄业务层：产生 `pairs`，其余全部委托。

### 前端

- **`types/opRecord.ts`**：`OpApplyItem / OpApplyResponse / OpRecordSummary<Extra> / OpRecordItem / OpRecordDetail<Extra> / OpRollbackCheck / OpRollbackResponse`。`SuffixRecordSummary = OpRecordSummary<{ targetSuffix }>`，`ModOpRecordSummary = OpRecordSummary<{ kind }>`。
- **`components/common/OpsPanel.vue`**：泛型面板，接收 preview / apply / checkRollback / rollback 回调 + 列定义 + `rows`；负责表格渲染、选择管理、按钮、确认对话框。三个面板（SuffixPanel / ModRenamePanel / ModOrganizePanel）都是薄包装。

### 新增一种同类业务的 checklist

假设要加"文件复制记录"：

1. 后端：建一张 `copy_op_records` + `copy_op_items`（结构对齐现有），在 `schema.rs` 加 `CREATE TABLE IF NOT EXISTS`。
2. 后端：在 `services::copy_tools::mod.rs` 里定义 `const COPY_TABLES: OpRecordTables = ...`，preview/apply 直接调 `op_pipeline::persist_apply_rename_pairs`，记录管理调 `op_record_repo`。
3. 后端：新建 `commands::copy_tools`，在 `lib.rs` `invoke_handler!` 注册。
4. 前端：`types/copyTools.ts` 用 `OpRecordSummary<Extra>` 特化；`services/copyTools.ts` 封装命令；store 复用 suffix/modTools 的写法；页面挂 `<OpsPanel>` 薄包装。

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

事件名字面量统一在 `src-tauri/src/constants.rs::events` 与前端 service 内，不要硬编码。

### 命令清单（按领域）

- **路径**：`normalize_input_paths`
- **去重 / 运行时 / 移动**：`start_dedup_task` / `pause_task` / `resume_task` / `stop_task` / `get_move_summary` / `apply_move_action`
- **预览**：`request_preview`
- **设置 / 数据库**：`get_settings` / `save_settings` / `set_theme_mode` / `get_db_info` / `set_custom_db_path` / `delete_database` / `get_cpu_count`
- **哈希记录**：`list_hash_records` / `load_hash_record` / `rename_hash_record` / `delete_hash_record`
- **后缀修改**：`preview_suffix_change` / `apply_suffix_change` / `list_suffix_change_records` / `get_suffix_change_record_detail` / `check_suffix_rollback` / `rollback_suffix_change` / `delete_suffix_change_record`
- **Mod 工具**：`preview_mod_rename` / `apply_mod_rename` / `preview_mod_organize` / `apply_mod_organize` / `list_mod_op_records` / `get_mod_op_record_detail` / `check_mod_op_rollback` / `rollback_mod_op` / `delete_mod_op_record` / `rename_mod_op_record` / `start_mod_scan_task` / `export_mod_scan_result`

新增命令：在 `commands/<mod>.rs` 写命令 → 在 `lib.rs` `invoke_handler!` 注册 → 前端 `services/<feature>.ts` 封装 → `types/<feature>.ts` 同步类型。

---

## 7. 代码风格规范

### 通用

- 所有前后端共享结构体必须 `#[serde(rename_all = "camelCase")]`，对应前端 TS 接口驼峰。
- 事件/状态/枚举等字符串常量统一放入 `constants.rs` / `constants/`，禁止硬编码。
- 不在运行态调用 `println!` / `eprintln!` / `console.log`；日志走 `events::emit_log`。`app_state::bootstrap` 之前的启动期错误可以 `eprintln!`（此时前端尚未连接）。

### Rust

- **模块注释**：每个 `.rs` 顶部写 `//!` 说明模块职责；IPC 契约或并发约定放在模块级。
- **函数/类型注释**：所有 `pub fn` / `pub struct` / `pub enum` 必须有 `///` 文档注释。短函数一行说明即可；涉及"并发、长路径、事务、事件顺序、取消语义"的函数要写明 WHY 与调用约定。
- **错误**：内部函数返回 `AppResult<T>`；命令层在 `Result<T, String>` 边界上把 `AppError` `.to_string()`。`rusqlite::Error` / `std::io::Error` / `serde_json::Error` 都有 `From` 实现，直接 `?` 传播即可。
- **路径**：所有 `std::fs` 调用前一律 `utils::path::to_extended_length_path`；展示或写库一律 `to_user_friendly_path`。
- **并发**：线程数统一 `op_pipeline::resolve_thread_count(db_path)`；需要自定义线程池时用 `rayon::ThreadPoolBuilder::build().install()` 本地化，不碰全局池。
- **数据库迁移**：新增表用 `CREATE TABLE IF NOT EXISTS`；新增列用 `let _ = conn.execute("ALTER TABLE ... ADD COLUMN ...")` 忽略重复列错误。不要改已有列。
- **记录型操作**：不要复制 suffix / mod_tools 作为模板，直接用 `op_record_repo` + `op_pipeline`。

### TypeScript / Vue

- **顶部 JSDoc**：每个 `.ts` / `.vue` 顶部一段 `/** ... */` 说明职责：service 指向对应后端命令，store 说明管理的状态域，组件说明 props 语义。
- **导出函数**：每个 `export function` 至少一行 JSDoc。复杂 composable / store action 解释 WHY（如日志批量缓冲、虚拟滚动边界条件）。
- **类型**：禁止业务代码出现 `any`（`OpsPanel` 的通用行回调可以接受 `unknown` + 具体子类型）；服务返回都有泛型类型。
- **Vue**：统一 `<script setup lang="ts">`；UI 组件使用 Element Plus（`el-` 前缀）；长列表一律 `VirtualTable`。"预览 → 应用 → 撤回"类面板一律通过 `common/OpsPanel.vue` 薄包装，不写自己的按钮组与确认对话框。
- **持久化**：跨会话状态用 `useStorage`；跨组件状态用 Pinia。
- **IPC**：组件内不要直接 `invoke` / `listen`，走 `services/<feature>.ts` 封装。

### 注释原则（WHY > WHAT）

- **写 WHY**：非直觉的约束、容易踩坑的边界、并发顺序、历史债务、调用契约。
- **不写 WHAT**：不要解释显而易见的代码。好的命名本身就是文档。
- **示例**：
  - ✅ `// 已提交的哈希任务不检查暂停状态，让它跑完`
  - ✅ `// bootstrap 阶段尚未初始化日志系统，只能直接打印到 stderr`
  - ❌ `// 创建一个 HashMap` / `// 遍历 paths`

---

## 8. 组件设计约定

- `VirtualTable`：固定行高虚拟滚动；支持列拖拽、全选、客户端/服务端分页、固定列、自定义插槽、ellipsis + tooltip。列定义类型 `VirtualColumn`。列宽拖拽用 `requestAnimationFrame` 节流。
- `OpsPanel`：通用"预览 → 应用 → 撤回"面板；顶部支持 `#topForm` 插槽放业务专属控件（如"目标后缀"输入）。
- `RealtimeLogPanel`：手写虚拟滚动，行高 30px；日志更新后 `scrollTop = scrollHeight` 实现自动跟随；滚到底部 ≤5px 自动开启跟随，离开底部 >50px 自动关闭。
- `DuplicateGroupTable`：`el-collapse` 手风琴；分段加载 (`renderLimits`) 处理大分组；全部勾选警告可关闭。

---

## 9. 性能与并发约定

- **高频事件（日志）**：后端批量发送（`PARTIAL_BATCH_SIZE = 30`），前端 `runtime` store 把事件先写入非响应式缓冲，每 150ms 批量刷入响应式 `logs` 数组，超过 `LOG_MAX_LENGTH = 3000` 裁剪旧数据。
- **去重流水线**：扫描 → 哈希（Semaphore 限流，许可 = 有效线程数 × 2）→ 分组 → 发送。流式 `mpsc` 通道边收边分组。
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
3. **记录型操作**：新增 preview→apply→rollback 模式的业务时，走 `op_record_repo + op_pipeline + OpsPanel`，**不要复制 suffix / mod_tools 当模板**。
4. **数据库迁移**：只允许 `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE ... ADD COLUMN`，不改已有列。迁移写在 `schema.rs::init_schema` 末尾。
5. **IPC 事件**：新增事件要在 `constants.rs::events` 登记，在 `services/events.rs` 写 emit 函数，在前端对应 store 的 `initEvents` 里监听。
6. **路径**：一切 `std::fs` 入参都要经过 `to_extended_length_path`；返回给前端的路径都要经过 `to_user_friendly_path`。
7. **并发**：线程数从 `op_pipeline::resolve_thread_count` 取；不要 inline 读 `settings.thread_count`。
8. **注释**：新增的 `pub fn` / `pub struct` / `pub enum` 必须带 `///`；新增的 TS `export function` / `defineStore` action 必须有 JSDoc。
9. **测试**：类型安全由 `cargo check` + `npx vue-tsc --noEmit` 双把关；UI 流程跑 `npm run tauri dev` 人工验证。

---

## 附录：常量速查

- 事件：`constants::events::{TASK_LOG, TASK_PROGRESS, TASK_STATE_CHANGED, TASK_FAILED, TASK_RESULT_PARTIAL, TASK_COMPLETED, MOVE_REPORT_READY, MOD_SCAN_COMPLETED}`
- 阶段：`constants::stages::{SCAN, HASH, MOD_SCAN}`
- 日志等级：`constants::log_level::{INFO, WARN, ERROR}`
- 保留策略：`constants::keep_policy::{NEWEST, OLDEST}`
- 主题：`constants::theme::{LIGHT, DARK, SYSTEM}`
- Mod 操作类型：`constants::mod_op_kind::{RENAME, ORGANIZE}`
- 哈希状态：`constants::hash_entry_status::ACTIVE`
- 数据库文件：`constants::db_file::{DEFAULT_NAME, WAL_EXT, SHM_EXT}`

前端对应常量在 `src/constants/`：`app.ts`（`LOG_MAX_LENGTH`, `LOG_FLUSH_INTERVAL`）、`task.ts`（分页与渲染阈值）、`theme.ts`。
