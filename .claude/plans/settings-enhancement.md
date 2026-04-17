# 配置中心增强：数据库路径 + 删除数据库 + 核心数配置

## 三个功能概述

1. **自定义 SQLite 数据库路径** — 用户可设置数据库存放位置，空值使用默认路径
2. **删除数据库** — 危险操作按钮，确认后删除并重建空数据库
3. **多线程核心数配置** — 用户设定并发核心数，后端 dedup 读取使用

## 关键设计决策

### 数据库路径存储方式
数据库路径**不能**存在 SQLite 里（鸡生蛋问题），需要在打开数据库之前就知道路径。
方案：在 `app_data_dir` 下创建 `fileflow_config.json` 外部配置文件，`lib.rs` 启动时先读它。

### 核心数存储方式
核心数**可以**存在 SQLite 的 `app_settings` 表里，因为它是在任务启动时才读取的。

### 数据库路径变更生效时机
修改后提示用户**重启应用后生效**，因为 `AppState.db_path` 不在锁后面，运行时变更不安全。

## 修改文件清单

| 文件 | 变更内容 | 功能 |
|------|---------|------|
| `src-tauri/src/external_config.rs` | **新建**：外部 JSON 配置读写 | F1 |
| `src-tauri/src/lib.rs` | 注册模块、启动时读外部配置、注册新命令 | F1,F2,F3 |
| `src-tauri/src/app_state.rs` | AppState 新增 `app_data_dir` 字段 | F1,F2 |
| `src-tauri/src/models.rs` | AppSettings 新增 `thread_count` 字段 | F3 |
| `src-tauri/src/config.rs` | 新增默认常量 | F3 |
| `src-tauri/src/db/schema.rs` | ALTER TABLE 迁移 `thread_count` 列 | F3 |
| `src-tauri/src/db/hash_repo.rs` | get/save_settings 增加 thread_count | F3 |
| `src-tauri/src/commands/settings.rs` | 新增 4 个命令：get_db_info/set_db_path/delete_database/get_cpu_count | F1,F2,F3 |
| `src-tauri/src/services/dedup.rs` | 并发数从设置读取 | F3 |
| `src/types/settings.ts` | 新增 DbPathInfo 接口、AppSettings 加 threadCount | F1,F3 |
| `src/services/settings.ts` | 新增 4 个 service 函数 | F1,F2,F3 |
| `src/stores/config.ts` | 新增 dbPathInfo 状态和 actions | F1,F2 |
| `src/views/SettingsPage.vue` | 新增三个 UI 区块 | F1,F2,F3 |

共 **1 个新文件 + 12 个修改文件**。

## 详细实现步骤

### 步骤 1：后端 — 外部配置模块（F1）

新建 `src-tauri/src/external_config.rs`：
- `ExternalConfig` 结构体：`{ db_path: Option<String> }`
- `load_config(app_data_dir)` → 读 JSON 文件，不存在返回默认
- `save_config(app_data_dir, config)` → 写 JSON
- `resolve_db_path(app_data_dir, config)` → 有值用自定义路径，否则用默认

### 步骤 2：后端 — AppState 加字段（F1）

`app_state.rs` 的 `AppState` 新增：
```rust
pub app_data_dir: PathBuf,
```

### 步骤 3：后端 — lib.rs 启动流程改造（F1）

在 `.setup()` 中：
1. 读 `external_config::load_config(&app_dir)`
2. 用 `resolve_db_path` 解析实际 db_path（若自定义路径的父目录不存在则回退默认）
3. 对解析后的路径执行 `init_schema`
4. 将 `app_data_dir` 和解析后的 `db_path` 一起传入 `AppState`

### 步骤 4：后端 — schema 迁移（F3）

`schema.rs` 在 `execute_batch` 之后增加：
```rust
let _ = conn.execute(
    "ALTER TABLE app_settings ADD COLUMN thread_count INTEGER NOT NULL DEFAULT 0",
    [],
);
```
`let _ =` 忽略 "duplicate column" 错误，这是标准 SQLite 迁移模式。

### 步骤 5：后端 — models + config + hash_repo（F3）

- `models.rs`：AppSettings 加 `thread_count: i32`，Default 值 `0`（0 = 自动）
- `config.rs`：新增 `DEFAULT_THREAD_COUNT: i32 = 0`
- `hash_repo.rs`：get_settings SELECT 加 `thread_count`，save_settings UPDATE 加 `thread_count`

### 步骤 6：后端 — 新命令（F1,F2,F3）

`commands/settings.rs` 新增：

1. `get_db_info` → 返回 `{ current_path, default_path, custom_path }`
2. `set_custom_db_path(path)` → 校验父目录存在 → 写 JSON → 提示重启
3. `delete_database` → 检查无运行中任务 → 删文件(含 WAL/SHM) → 重建 schema → 清内存
4. `get_cpu_count` → 返回 `num_cpus::get()`

### 步骤 7：后端 — dedup 并发读设置（F3）

`dedup.rs` 第 108 行改为：
```rust
let settings = hash_repo::get_settings(&app_state.db_path).unwrap_or_default();
let concurrency = if settings.thread_count > 0 {
    (settings.thread_count as usize).min(num_cpus::get()).max(1) * 2
} else {
    num_cpus::get().max(2) * 2
};
```

### 步骤 8：前端 — 类型 + 服务 + Store

- `types/settings.ts`：AppSettings 加 `threadCount`，新增 `DbPathInfo` 接口
- `services/settings.ts`：新增 `getDbInfo`、`setCustomDbPath`、`deleteDatabase`、`getCpuCount`
- `stores/config.ts`：state 加 `threadCount: 0`，新增 `loadDbInfo`、`deleteDb` 等 actions

### 步骤 9：前端 — SettingsPage UI

SettingsPage.vue 新增三个区块：

1. **数据库路径区块**（在现有卡片内，主题之后）：
   - 显示当前数据库路径（只读）
   - 输入框设置自定义路径
   - 提示"修改后需重启应用生效"

2. **处理核心数区块**（在数据库路径之后）：
   - `el-slider` 0~CPU总数，0 显示"自动（全部核心）"
   - 标签显示 CPU 总核心数供参考

3. **危险操作区块**（最底部，独立 el-card）：
   - 红色按钮"删除数据库"
   - 点击弹出 `ElMessageBox.confirm` 确认对话框，警告所有数据将永久删除
   - 确认后执行删除 + 重新加载设置
