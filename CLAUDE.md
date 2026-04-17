# FileFlow Desktop — 项目规范提示词

> 本文档是 FileFlow Desktop 项目的完整架构规范。在任何 AI 辅助开发场景中，将此文档作为系统提示词或上下文提供，以确保代码修改遵循现有架构约定。

---

## 1. 项目概述

FileFlow Desktop 是一个 **Windows 桌面文件处理平台**，基于 Tauri 2 构建。核心功能包括：

- **文件去重**：扫描指定文件夹，使用 BLAKE3 哈希算法识别重复文件，支持暂停/恢复/停止，可将重复文件移动到目标目录
- **后缀批量修改**：批量修改文件扩展名，支持预览、应用、撤回、历史记录管理
- **文件预览**：支持文本、图片、压缩包内容预览
- **记录管理**：哈希索引记录和后缀修改记录的增删改查

---

## 2. 技术栈

### 前端
| 技术 | 版本 | 用途 |
|------|------|------|
| Vue 3 | ^3.5 | UI 框架（Composition API + `<script setup>`） |
| TypeScript | ^5.9 | 类型安全 |
| Pinia | ^3.0 | 状态管理（Options API 风格 store） |
| Element Plus | ^2.13 | UI 组件库 |
| VueUse | ^14.2 | 组合式工具（useStorage、useDark、useVirtualList 等） |
| Vue Router | ^5.0 | 路由（Hash 模式） |
| Vite | ^7.3 | 构建工具 |
| @tauri-apps/api | ^2.10 | Tauri IPC 通信 |
| @tauri-apps/plugin-dialog | ^2.6 | 原生文件夹选择对话框 |

### 后端（Rust）
| 技术 | 版本 | 用途 |
|------|------|------|
| Tauri | 2.10 | 桌面应用框架（含 protocol-asset） |
| tokio | 1.49 | 异步运行时（rt-multi-thread, macros, sync, time） |
| rusqlite | 0.38 | SQLite 数据库（bundled 模式） |
| blake3 | 1.8 | 文件哈希算法 |
| walkdir | 2.5 | 递归目录遍历 |
| rayon | 1.10 | 并行迭代器 |
| uuid | 1.21 | UUID v4 生成 |
| chrono | 0.4 | 时间处理 |
| num_cpus | 1.17 | CPU 核心数检测 |
| serde / serde_json | 1 | 序列化/反序列化 |
| thiserror | 2.0 | 错误类型派生 |
| zip | 8.1 | 压缩包读取（预览用） |
| image | 0.25 | 图片处理（预览用） |

---

## 3. 项目结构

```
fileflow-desktop/
├── src/                          # 前端源码
│   ├── App.vue                   # 根组件（侧边栏 + 路由出口）
│   ├── main.ts                   # 入口
│   ├── router/
│   │   ├── index.ts              # createRouter (Hash 模式)
│   │   └── routes.ts             # 路由表
│   ├── views/
│   │   ├── TaskPage.vue          # 任务中心（左栏：路径输入+日志，右栏：功能Tab）
│   │   ├── SettingsPage.vue      # 配置中心
│   │   └── RecordManagePage.vue  # 记录管理
│   ├── components/
│   │   ├── DedupPanel.vue        # 去重功能面板
│   │   ├── DuplicateGroupTable.vue # 重复文件分组表格（el-collapse + el-table）
│   │   ├── SuffixPanel.vue       # 后缀批量修改面板
│   │   ├── RealtimeLogPanel.vue  # 实时日志（手写虚拟滚动，固定行高）
│   │   ├── TaskControlPanel.vue  # 任务控制按钮（开始/暂停/继续/停止）
│   │   ├── PreviewPanel.vue      # 文件预览悬浮面板
│   │   ├── MoveConfirmDialog.vue # 移动确认对话框
│   │   ├── MoveReportDialog.vue  # 移动报告对话框
│   │   ├── RecordDetailDrawer.vue # 哈希记录详情抽屉
│   │   └── common/
│   │       └── VirtualTable.vue  # 通用虚拟表格组件
│   ├── stores/
│   │   ├── runtime.ts            # 任务运行时状态（日志、进度、IPC事件监听）
│   │   ├── task.ts               # 去重任务结果状态
│   │   ├── record.ts             # 哈希索引记录管理
│   │   ├── config.ts             # 应用配置
│   │   ├── suffix.ts             # 后缀修改状态
│   │   └── preview.ts            # 预览状态
│   ├── services/                 # Tauri IPC 调用封装
│   │   ├── tauri.ts              # invokeCmd / onEvent 基础封装
│   │   ├── task.ts               # 去重/运行时/移动相关命令
│   │   ├── settings.ts           # 设置相关命令
│   │   ├── record.ts             # 记录相关命令
│   │   ├── suffix.ts             # 后缀修改相关命令
│   │   └── preview.ts            # 预览命令
│   ├── types/                    # TypeScript 类型定义
│   │   ├── common.ts             # TaskStatus, TaskLogPayload, TaskProgressPayload
│   │   ├── task.ts               # DedupConfig, FileEntry, DuplicateGroup
│   │   ├── settings.ts           # AppSettings, DbPathInfo
│   │   ├── record.ts             # HashIndexRecord, HashIndexRecordSummary
│   │   ├── moveReport.ts         # MoveReport, MoveSummary, MoveActionResponse
│   │   ├── suffix.ts             # Suffix* 系列类型
│   │   ├── preview.ts            # PreviewPayload
│   │   └── virtualTable.ts       # VirtualColumn, PaginationConfig, RenderColumn
│   ├── composables/
│   │   ├── useTheme.ts           # 主题切换（light/dark/system）
│   │   └── usePathNormalize.ts   # 路径规范化（调用后端 + 警告提示）
│   ├── utils/
│   │   ├── path.ts               # uniquePaths, stripWindowsExtendedPrefix
│   │   ├── format.ts             # formatBytes, formatTimestamp
│   │   └── mapper.ts             # mapGroup (后端数据映射)
│   └── constants/
│       ├── app.ts                # LOG_MAX_LENGTH, LOG_FLUSH_INTERVAL
│       └── task.ts               # 分页/渲染限制常量
│
├── src-tauri/                    # Rust 后端源码
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs               # 入口（调用 lib::run）
│       ├── lib.rs                # Tauri Builder 配置、命令注册、AppState 初始化
│       ├── models.rs             # 所有数据模型（serde, camelCase）
│       ├── config.rs             # 常量配置
│       ├── error.rs              # AppError 枚举 + AppResult 类型别名
│       ├── app_state.rs          # AppState（db_path, tasks, task_results）+ TaskRuntime
│       ├── external_config.rs    # 外部 JSON 配置（数据库路径，独立于 SQLite）
│       ├── commands/             # Tauri 命令（#[tauri::command]）
│       │   ├── mod.rs
│       │   ├── dedup.rs          # start_dedup_task
│       │   ├── runtime.rs        # pause/resume/stop_task
│       │   ├── move_file.rs      # get_move_summary, apply_move_action
│       │   ├── path.rs           # normalize_input_paths
│       │   ├── preview.rs        # request_preview
│       │   ├── settings.rs       # get/save_settings, db管理, cpu_count
│       │   ├── records.rs        # 哈希记录 CRUD
│       │   └── suffix.rs         # 后缀修改全套命令
│       ├── services/             # 业务逻辑层
│       │   ├── mod.rs
│       │   ├── dedup.rs          # 去重核心逻辑（扫描+哈希+分组）
│       │   ├── move_file.rs      # 文件移动逻辑
│       │   ├── record.rs         # 记录业务逻辑
│       │   ├── preview.rs        # 文件预览逻辑
│       │   └── suffix.rs         # 后缀修改业务逻辑
│       ├── db/                   # 数据库层
│       │   ├── mod.rs
│       │   ├── schema.rs         # init_schema（DDL + 迁移）
│       │   ├── hash_repo.rs      # 哈希记录 CRUD（6个函数）
│       │   ├── settings_repo.rs  # app_settings 表 CRUD
│       │   ├── move_repo.rs      # 移动报告持久化
│       │   └── suffix_repo.rs    # 后缀修改记录 CRUD
│       └── utils/                # Rust 工具函数
│           ├── mod.rs
│           └── path.rs           # Windows 长路径处理
```

---

## 4. 架构约定

### 4.1 前后端通信
- **IPC 模式**：前端通过 `invokeCmd<T>(command, payload)` 调用后端 `#[tauri::command]` 函数
- **事件模式**：后端通过 `app_handle.emit(event_name, payload)` 向前端推送事件，前端通过 `onEvent<T>(event_name, callback)` 监听
- **命名转换**：Rust 使用 `snake_case`，前端使用 `camelCase`，通过 `#[serde(rename_all = "camelCase")]` 自动转换

### 4.2 IPC 事件清单
| 事件名 | 方向 | 载荷 | 用途 |
|--------|------|------|------|
| `task_log` | 后端→前端 | `TaskLogPayload` | 实时日志推送 |
| `task_progress` | 后端→前端 | `TaskProgressPayload` | 进度更新 |
| `task_state_changed` | 后端→前端 | `{ taskId, status }` | 任务状态变更 |
| `task_failed` | 后端→前端 | `{ taskId, message }` | 任务失败通知 |
| `task_result_partial` | 后端→前端 | `{ taskId, groups }` | 增量结果推送 |
| `task_completed` | 后端→前端 | `{ taskId, groups }` | 任务完成 + 最终结果 |
| `move_report_ready` | 后端→前端 | `{ taskId, report, updatedGroups }` | 移动完成报告 |

### 4.3 状态管理
- 使用 **Pinia Options API** 风格（`state()` + `actions` + `getters`）
- Store 文件放在 `src/stores/` 下
- 持久化使用 `useStorage`（VueUse，底层 localStorage）
- 日志采用**批量缓冲**模式：高频 IPC 事件写入非响应式 `_logBuffer`，定时器（150ms 间隔）批量刷入响应式 `logs` 数组

### 4.4 数据库
- **SQLite**（rusqlite，WAL 模式，bundled）
- 每次操作**独立开连接**（`Connection::open`），不使用连接池
- **Schema 迁移**：`init_schema` 中先 `CREATE TABLE IF NOT EXISTS`，再用 `let _ = conn.execute("ALTER TABLE ... ADD COLUMN ...")` 忽略重复列错误
- **单行设置表**：`app_settings` 使用 `CHECK(id=1)` 约束确保只有一行
- **外部配置**：数据库路径存储在 JSON 文件（`fileflow_config.json`）中，因为打开数据库前需要知道路径（鸡生蛋问题）

### 4.5 并发模型
- 去重任务在 `tokio::spawn` 中运行
- 文件哈希使用 `tokio::spawn_blocking`，通过 `tokio::sync::Semaphore` 控制并发数
- 并发数 = `settings.thread_count`（0 = 自动，取 `num_cpus::get()`），实际信号量许可数 = 并发数 × 2
- 任务控制通过 `AtomicBool`（`paused` / `cancelled`）实现暂停/取消

### 4.6 Windows 长路径
- 所有后端路径处理需考虑 `\\?\` 前缀
- 前端显示时使用 `stripWindowsExtendedPrefix()` 去除前缀
- 路径比较时统一使用正斜杠 `/` 并做前缀匹配

---

## 5. 核心数据模型

### 前端 TypeScript

```typescript
// types/common.ts
type TaskStatus = "Idle" | "Running" | "Paused" | "Cancelled" | "Completed" | "Failed";

interface TaskLogPayload {
  taskId: string;
  level: "INFO" | "WARN" | "ERROR";
  message: string;
  filePath?: string;
}

interface TaskProgressPayload {
  taskId: string;
  stage: string;
  processed: number;
  total: number;
  percent: number;
}

// types/task.ts
interface DedupConfig {
  keepPolicy: "newest" | "oldest";
  moveTargetPath?: string | null;
  autoSelectEnabled: boolean;
  saveRecordEnabled: boolean;
  useLastRecordEnabled: boolean;
  selectedRecordId?: string | null;
  includeCurrentFolderDuplicates: boolean;
  recordName?: string | null;
}

interface FileEntry {
  absPath: string;
  size: number;
  mtime: number;
  ctime: number;
  hash?: string;
  selectedForMove?: boolean;
  fromHistory?: boolean;
}

interface DuplicateGroup {
  groupId: string;
  hash: string;
  files: FileEntry[];
}

// types/settings.ts
interface AppSettings {
  keepPolicy: "newest" | "oldest";
  moveTargetPath?: string | null;
  saveRecordEnabled: boolean;
  useLastRecordEnabled: boolean;
  includeCurrentFolderDuplicates: boolean;
  themeMode: "light" | "dark" | "system";
  threadCount: number; // 0 = 自动
}

interface DbPathInfo {
  currentPath: string;
  defaultPath: string;
  customPath: string | null;
}

// types/virtualTable.ts
interface VirtualColumn {
  key: string;
  label: string;
  width?: number;
  minWidth?: number;
  ellipsis?: boolean;
  slotName?: string;
  formatter?: (row: any, value: any, index: number) => string;
  fixed?: "left" | "right";
  resizable?: boolean;
}
```

### 后端 Rust

```rust
// models.rs — 所有结构体均使用 #[serde(rename_all = "camelCase")]

// 任务相关
pub enum TaskStatus { Idle, Running, Paused, Cancelled, Completed, Failed }
pub struct DedupConfig { keep_policy, move_target_path, auto_select_enabled, ... }
pub struct FileEntry { abs_path, size, mtime, ctime, hash, selected_for_move, from_history }
pub struct DuplicateGroup { group_id, hash, files: Vec<FileEntry> }
pub struct TaskLogPayload { task_id, level, message, file_path }
pub struct TaskProgressPayload { task_id, stage, processed, total, percent }

// 设置
pub struct AppSettings { keep_policy, move_target_path, save_record_enabled, ..., thread_count }

// 记录
pub struct HashIndexRecord { record_id, record_name, created_at, source_paths, entries }
pub struct HashIndexRecordSummary { record_id, record_name, created_at, source_paths, entry_count }
pub struct HashIndexEntry { hash, file_path, file_size, mtime, ctime, status }

// 移动
pub struct MoveSummary { target_dir, total_selected, total_size }
pub struct MoveReport { report_id, task_id, created_at, target_dir, ..., success_items, failed_items }
pub struct MoveActionResponse { report, updated_groups }

// 后缀修改
pub struct SuffixPreviewItem { old_path, new_path, will_rename_conflict }
pub struct SuffixApplyResponse { record_id, record_name, total, success, failed, items }
pub struct SuffixRecordSummary { record_id, record_name, target_suffix, created_at, ... }
pub struct SuffixRecordDetail { summary, items: Vec<SuffixRecordItem> }
pub struct SuffixRollbackResponse { record_id, total_selected, success, failed, skipped_missing, items }

// 错误处理
pub enum AppError { Db(String), Io(String), InvalidInput(String), TaskNotFound, NotFound(String), Internal(String) }
pub type AppResult<T> = Result<T, AppError>;

// 应用状态
pub struct AppState { app_data_dir: PathBuf, db_path: PathBuf, tasks: Mutex<HashMap<...>>, task_results: Mutex<HashMap<...>> }
pub struct TaskRuntime { paused: AtomicBool, cancelled: AtomicBool, status: Mutex<TaskStatus> }

// 外部配置
pub struct ExternalConfig { db_path: Option<String> }
```

---

## 6. Tauri 命令注册清单

```rust
// lib.rs invoke_handler
commands::path::normalize_input_paths
commands::dedup::start_dedup_task
commands::runtime::pause_task
commands::runtime::resume_task
commands::runtime::stop_task
commands::move_file::get_move_summary
commands::move_file::apply_move_action
commands::preview::request_preview
commands::settings::get_settings
commands::settings::save_settings
commands::settings::set_theme_mode
commands::settings::get_db_info
commands::settings::set_custom_db_path
commands::settings::delete_database
commands::settings::get_cpu_count
commands::records::list_hash_records
commands::records::load_hash_record
commands::records::rename_hash_record
commands::records::delete_hash_record
commands::suffix::preview_suffix_change
commands::suffix::apply_suffix_change
commands::suffix::list_suffix_change_records
commands::suffix::get_suffix_change_record_detail
commands::suffix::check_suffix_rollback
commands::suffix::delete_suffix_change_record
commands::suffix::rollback_suffix_change
```

---

## 7. 组件设计约定

### 7.1 通用规则
- 所有 Vue 组件使用 `<script setup lang="ts">` + Composition API
- UI 组件库统一使用 Element Plus，前缀 `el-`
- 持久化状态使用 `useStorage`（VueUse）
- 长列表使用 `VirtualTable` 组件（固定行高虚拟滚动）或手写虚拟滚动

### 7.2 VirtualTable 组件
- 路径：`src/components/common/VirtualTable.vue`
- 支持功能：虚拟滚动、列拖拽调整宽度（`resizable`）、全选/取消全选（`selectable`）、客户端/服务端分页、固定列（`fixed`）、自定义插槽（`slotName`）、ellipsis + tooltip
- 列定义类型：`VirtualColumn`
- 列宽拖拽使用 `requestAnimationFrame` 节流

### 7.3 RealtimeLogPanel 组件
- 手写虚拟滚动（不使用 useVirtualList），统一行高 30px
- 自动跟随：日志更新时双 `nextTick` 后 `scrollTop = scrollHeight`
- 滚到底部（≤5px）自动开启跟随，离开底部（>50px）自动关闭
- 行内只显示文件名，tooltip 显示完整路径
- 支持清空日志

### 7.4 DuplicateGroupTable 组件
- 使用 `el-collapse`（手风琴模式）展示分组
- 每组内使用 `el-table` 展示文件列表
- 支持"保留最新/最旧"、"保留此文件"操作
- 分段加载（`renderLimits`）处理大分组
- 全部勾选警告（`showAllSelectedWarning`，可关闭）

---

## 8. 编码规范

### 8.1 前端
- TypeScript 严格模式
- 类型定义集中在 `src/types/` 目录
- 服务层（`src/services/`）只做 IPC 调用封装，不含业务逻辑
- Store 处理业务逻辑和状态管理
- 组件内避免直接调用 `invoke`，统一通过 service 层

### 8.2 后端
- 三层架构：`commands`（参数解析/权限）→ `services`（业务逻辑）→ `db`（数据访问）
- 所有 Tauri 命令返回 `Result<T, String>`（AppError 转 String）
- 数据库 repo 函数接收 `db_path: &Path` 参数，内部开连接
- Serde 统一使用 `rename_all = "camelCase"` 确保前后端字段名一致
- 错误处理使用 `thiserror` 派生的 `AppError` 枚举

### 8.3 性能约定
- 高频事件（日志）使用非响应式缓冲 + 定时批量刷入
- 长列表使用虚拟滚动
- 列拖拽使用 RAF 节流
- 大文件操作使用 `tokio::spawn_blocking`
- 并发控制使用 `Semaphore`

---

## 9. 主题系统

- 三种模式：`light` / `dark` / `system`（跟随系统）
- 使用 VueUse 的 `useDark` + `usePreferredDark`
- Element Plus 暗色主题通过 `html.dark` CSS class 切换
- 持久化到 `localStorage`（`useStorage`）+ SQLite `app_settings.theme_mode`

---

## 10. 路由结构

| 路径 | 组件 | 功能 |
|------|------|------|
| `/` | TaskPage | 任务中心（去重 + 后缀修改） |
| `/settings` | SettingsPage | 配置中心（设置 + 数据库管理） |
| `/records` | RecordManagePage | 记录管理（哈希记录 + 后缀记录） |

---

## 11. 重要实现细节

### 11.1 日志批量缓冲
```
IPC 事件 → _logBuffer (非响应式数组)
          → setInterval 每 150ms 批量刷入
          → this.logs = merged (单次响应式更新)
          → 超过 3000 条时 slice 裁剪旧数据
```

### 11.2 外部配置（鸡生蛋问题）
SQLite 数据库路径不能存在 SQLite 中，所以使用独立的 JSON 文件：
```
app_data_dir/fileflow_config.json → { "db_path": "..." }
启动时：load_config → resolve_db_path → init_schema
```

### 11.3 去重流程
```
前端 startDedupTask(paths, config)
→ 后端 spawn 异步任务
  → 阶段1: scan (walkdir 递归扫描)
  → 阶段2: hash (BLAKE3, Semaphore 控制并发)
  → 阶段3: group (按 hash 分组)
  → 过程中 emit task_log / task_progress / task_result_partial
  → 完成时 emit task_completed
前端 store 监听事件，增量更新 resultGroups
```

### 11.4 任务完成后自动刷新记录
`task_completed` 和 `move_report_ready` 事件回调中自动调用 `recordStore.refresh()`。

---

## 12. 修改代码时的注意事项

1. **前后端模型同步**：修改 `models.rs` 中的结构体时，必须同步修改对应的 TypeScript 类型
2. **命令注册**：新增 Tauri 命令必须在 `lib.rs` 的 `invoke_handler` 中注册
3. **数据库迁移**：新增列使用 `ALTER TABLE ADD COLUMN`（`let _ =` 忽略重复），不要修改 `CREATE TABLE`
4. **IPC 事件**：新增事件需要在前端 store 的 `initEvents` 中监听
5. **Windows 路径**：所有路径显示使用 `stripWindowsExtendedPrefix`，路径比较使用正斜杠归一化
6. **性能**：避免在高频更新的数据上使用深层响应式监听，使用批量缓冲模式
7. **UI 组件**：长列表优先使用 `VirtualTable`；如结构不适合（如分层数据），保持 `el-table` + 分段加载
8. **类型安全**：所有新增代码必须通过 `vue-tsc --noEmit` 编译检查
