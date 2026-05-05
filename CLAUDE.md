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
- **空文件夹清理**：递归预览空目录，按深度从深到浅删除并写入可撤回记录；撤回会重新创建空目录，默认不删除任务输入根目录。`preview_empty_dirs` 单条路径访问失败（不存在 / 无权限 / 不是目录）只跳过该条路径，不再 abort 整批扫描——与 dedup / suffix 的 warn-and-continue 一致；只有全部路径都失败时才返回错误。
- **Mod 工具**：针对 Illusion 系列 `.zipmod` 的六类操作——
  - **重命名**：按 manifest.xml 的 `guid/author/version` 生成 `[author] guid-version.zipmod`；同批次撞名时按稳定顺序自动分配 ` (N)` 冲突后缀。
  - **归类**：按文件名首个 `[...]` 建子目录归类。
  - **重复 MOD 检查**：按 `guid + author + version` 分组找重复，默认每组保留最新；删除时把文件移动到集中备份目录 `<backup_root>/<record_id>/<原文件名>`，备份路径写入记录可撤回。
  - **不同版本 MOD 检查**：按 `guid + author` 分组找多个 `version`，默认保留最高版本；删除走同样的集中备份目录方案。
  - **移除版本限制（modify）**：就地重写 zip 去掉指定 `<game>KEYWORD</game>` 标签；启用回滚时把原文件 `std::fs::copy` 备份到 `<backup_root>/<record_id>/<原文件名>` 后再原子替换原文件。
  - **版本限制扫描**：长任务，扫描结果勾选后由"移除版本限制"落盘为 Mod 操作记录。

  这三类备份型操作受用户设置 `mod_rollback_enabled` 控制：默认开启，关闭后直接 `remove_file` / in-place 改写不留备份，记录主表 `rollback_enabled = 0`，UI 上"撤回"按钮置灰、后端命令也会拒绝。备份目录由 `mod_backup_dir` 配置，未配置时落到 `<exe_dir>/mod-backups`；与源文件跨卷时 `op_pipeline::rename_or_copy_delete` 自动退化为 copy + delete 兜底。重命名 / 归类不进入备份概念，永远可撤回。
- **文件预览**：文本 / 图片 / 压缩包内容查看；压缩包预览列出条目路径、大小、目录标记、修改时间。
- **Pixiv 标签整理**：按文件名里的 8~9 位 PID 调用 `https://www.pixiv.net/ajax/illust/<pid>` 拿 tag，用虚拟表把每个 tag 渲染成 chip 气泡；点击 chip 把图移到 `<输出目录>/<tag>/`，行保留以便往别的 tag 文件夹继续移。设置中可配置接口 base、排除 tag、Pixiv Cookie、本地 tag 翻译表；排除 tag 与本地翻译表都支持从 JSON / 文本文件导入，导出为 JSON。`pixivUseTranslation` 开关在配置中心和任务面板顶部都暴露，开启后本地翻译表 `pixivLocalTagTranslations[原tag]` 优先作为 chip 显示文本与目录名；没有本地译名时再用 Pixiv 响应里的 `translation.en`；两者都缺失则回落原 tag。每行右侧"译名"列还有三态 segmented 控件（`global` / `original` / `translated`）做单行覆盖。**chip 列展示走 render-time 函数 `displayTagsForRow(row)`**：每次模板渲染时同步访问 `pixivUseTranslation` / `pixivExcludedTags` / `pixivLocalTagTranslations` / `row.tags` / `row.translations` / `row.useTranslationOverride`，依赖直接落在组件 render effect 上；排除判断始终先匹配原 tag，同时兼容匹配当前展示字符串，所以原 tag 被排除时切到译名也不会露出；同时把 `pixivUseTranslation + pixivExcludedTags + pixivLocalTagTranslations` 折成 `tagSlotRefreshKey` 传给 VirtualTable 的 `slotRefreshKey`，让全局开关 / 排除项 / 本地翻译变化时可见行 slot cell 强制重建，避免虚拟滚动复用旧 chip 子树。VirtualTable 的 v-for 只对**可见行**执行，per-row 计算只在屏内十几行上跑，比 panel 级 watchEffect 全表重算更省。**同 PID 多张图**（`_p0..._pN`）：store 用 `_pidIndex: Map<string, number[]>` 把每个 PID 映射到所有索引，partial 一到达 `_commitPending` 就把结果应用到整组同 PID 行；行级操作（移动 / 单行重试 / 译名覆盖）按 `absPath` 定位（`_pathIndex: Map<string, number>`，moveByTag 完成后旧 absPath 删除、新 absPath 写入同 idx）。`pixivPartialFlushIntervalMs` 控制前端的 partial 合并刷新节奏：0 = 实时（默认）；>0 = `setTimeout` 节流到固定间隔（done 终态会立刻 flush 一次不被拖延）。`pixivRateLimitPerMinute` 限制每分钟最大请求数（默认 60 = 1 req/s），所有并发 worker 与重试共享一条 next-slot 节流队列，防止瞬时打穿被 Pixiv 拉黑。后端 worker **每条 PID 完成都会发一行 task_log**（成功 INFO + tag 数 + 译名数；失败 WARN + PID + 错误首行），日志面板能看到具体哪个 ID 出错；面板顶部"重试失败 (N)"按钮一键调 `store.retryFailed`，按 PID 去重后串行重试所有 error 行（每条 fetchPixivTagSingle 也走共享限速队列，不会瞬时打穿）。**取值约定**（对照 Pixiv 响应 `body.tags.tags[*]`）：每个 item 的 `tag` 字段就是原 tag 字符串，可选的 `translation.en` 是社区英译；后端 `parse_pixiv_tag_payload` 把所有"有 translation.en"的 tag 汇总成 `original → en` Map 一并发回前端，前端按"开关 + 行级覆盖 + 本地翻译优先"决定 chip 显示原 tag 还是译名。可选缩略图列开关；开启后图片列默认排到最前面，"文件"列上的 hover 浮窗自动去掉。表头支持列自定义（显示 / 顺序 / 左固定，持久化键 `pixiv:tags`），row-key 用 `absPath`（同 PID 多行时 PID 不再唯一，用作 row-key 会让虚拟表把多行视作"同一行"互相覆盖）。HTTP 走后端 `reqwest`（rustls + webpki bundled 根证书）规避 CORS 与系统证书库依赖。无可撤回记录（一次扫描会产生多次点击的工作流，不适合每次点击都写一条记录）。store 端 partial / retry / moveByTag **必须用对象替换写 `this.rows[idx] = {...}`**，不要 in-place 改嵌套字段 —— 嵌套 reactive proxy 在多次写入下偶发不触发依赖（与 `moveByTag` 同一坑）。
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
| quick-xml | 0.39 | manifest.xml 解析（`BytesText::decode` 替代已移除的 `unescape`） |
| encoding_rs | 0.8 | manifest 编码回退（UTF-8 / GBK / Shift_JIS） |
| zip | 8.1 | 压缩包读取、`raw_copy_file` 零重编码重写 |
| image | 0.25 | 图片元信息预览 |
| uuid | 1.21 | UUID v4 |
| chrono | 0.4 | 时间处理 |
| num_cpus | 1.17 | CPU 核心数 |
| serde / serde_json | 1 | 序列化，`rename_all = "camelCase"` |
| thiserror | 2.0 | `AppError` 派生 |
| reqwest | 0.13 | Pixiv tag HTTP 客户端（特性 `rustls + gzip + json + socks`，0.13 已默认 rustls + aws-lc-rs，避开 OpenSSL 系统依赖；`socks` 让用户能用 SOCKS5 代理；旧名 `rustls-tls` 在 0.12 才有） |
| regex | 1 | Pixiv 文件名 PID 提取（`\d{8,9}`） |

---

## 3. 目录结构

```
fileflow-desktop/                        # 仓库目录名（可手动改为 kk-file-tool）
├── src/                                 # 前端
│   ├── App.vue / main.ts
│   ├── router/{index,routes}.ts
│   ├── views/
│   │   ├── TaskPage.vue                 # 任务中心：路径+日志 / 去重 / 后缀 / 空文件夹 / Pixiv 标签 / Mod 工具
│   │   ├── SettingsPage.vue             # 配置中心（左侧导航 + 右侧滚动 + IntersectionObserver 高亮）
│   │   └── RecordManagePage.vue         # 四类记录管理（Mod 记录按 kind 过滤）
│   ├── components/
│   │   ├── DedupPanel.vue               # 去重面板（独立，不走 OpsPanel）
│   │   ├── SuffixPanel.vue              # 薄包装 → OpsPanel
│   │   ├── EmptyDirsPanel.vue           # 薄包装 → OpsPanel
│   │   ├── ModRenamePanel.vue           # 薄包装 → OpsPanel
│   │   ├── ModOrganizePanel.vue         # 薄包装 → OpsPanel
│   │   ├── ModDuplicatePanel.vue        # 薄包装 → ModGroupPanel(kind=duplicate)
│   │   ├── ModVersionPanel.vue          # 薄包装 → ModGroupPanel(kind=version)
│   │   ├── ModGroupPanel.vue            # 重复/不同版本 MOD 分组检查共用面板
│   │   ├── ModScanPanel.vue             # 扫描 + 勾选 + modify
│   │   ├── ModToolsPanel.vue            # 五个 Mod 子 Tab 容器（TabBar + v-show）
│   │   ├── PixivTagPanel.vue            # Pixiv 标签整理（动态 tag 列 + 单元格点击移动）
│   │   ├── RealtimeLogPanel.vue         # 手写虚拟滚动日志
│   │   ├── DuplicateGroupTable.vue
│   │   ├── PreviewPanel.vue / PathPreviewLink.vue / MoveConfirmDialog.vue / MoveReportDialog.vue
│   │   ├── RecordTab.vue                # 记录管理页通用 tab（列表 + 详情 + 撤回 + 批删）
│   │   ├── RecordDetailDrawer.vue / TaskControlPanel.vue
│   │   └── common/
│   │       ├── Panel.vue                # 自有卡片原语，替代 el-card
│   │       ├── TabBar.vue               # 自有段式切换，替代 el-tabs
│   │       ├── VirtualTable.vue         # 通用虚拟表
│   │       └── OpsPanel.vue             # 通用"预览→应用→撤回"面板
│   ├── stores/                          # Pinia Options-style
│   │   └── runtime.ts / task.ts / record.ts / config.ts / preview.ts /
│   │       suffix.ts / emptyDirs.ts / modTools.ts / pixivTag.ts /
│   │       _opRecordCrud.ts             # 通用可撤回记录 CRUD action 工厂
│   ├── services/                        # IPC 封装
│   │   └── tauri.ts / task.ts / settings.ts / record.ts / preview.ts /
│   │       suffix.ts / emptyDirs.ts / modTools.ts / pixivTag.ts
│   ├── types/
│   │   ├── common.ts / task.ts / settings.ts / record.ts / moveReport.ts /
│   │   │   preview.ts / virtualTable.ts
│   │   ├── opRecord.ts                  # 通用可撤回操作记录类型
│   │   └── suffix.ts / emptyDirs.ts / modTools.ts / pixivTag.ts
│   ├── composables/
│   │   ├── useTheme.ts                  # 主题切换
│   │   ├── usePathNormalize.ts          # 路径规范化 + 警告弹窗
│   │   ├── useFolderPicker.ts           # 系统目录选择弹窗封装
│   │   ├── useDangerConfirm.ts          # 缺失路径 / 危险操作确认文案封装
│   │   └── useLocalLongTask.ts          # 前端预生成 task_id 的长任务启动封装
│   ├── utils/
│   │   ├── path.ts                      # uniquePaths / stripWindowsExtendedPrefix / baseName
│   │   ├── format.ts / error.ts / clipboard.ts
│   │   ├── groupUpsert.ts               # groupId 增量合并 + 排序
│   │   └── taskId.ts                    # 前端本地 task_id 生成
│   └── constants/
│       └── app.ts / task.ts / theme.ts / preview.ts / recordColumns.ts
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
│   │       settings.rs / suffix.rs / empty_dirs.rs / mod_tools.rs / pixiv_tag.rs
│   ├── db/                              # 数据访问层
│   │   ├── mod.rs / schema.rs
│   │   ├── op_record_repo.rs            # 通用"操作记录"仓储（suffix / mod_op / empty_dir 共用）
│   │   └── hash_repo.rs / move_repo.rs / settings_repo.rs
│   ├── services/                        # 业务逻辑
│   │   ├── mod.rs / events.rs / logging.rs
│   │   ├── op_pipeline.rs               # 通用 preview→apply→rollback 流水线
│   │   ├── suffix.rs / empty_dirs.rs / dedup.rs / move_file.rs / preview.rs
│   │   ├── pixiv_tag.rs                 # Pixiv tag 拉取长任务 + 单条移动（reqwest + tokio Semaphore）
│   │   └── mod_tools/
│   │       ├── mod.rs                   # 记录查询/删除/重命名/撤回（映射到 op_pipeline）
│   │       ├── backup.rs                # Mod 备份目录解析与备份对构造（cleanup / modify 共享 prepare_mod_backup 入口）
│   │       ├── rename.rs / organize.rs  # 纯 rename → op_pipeline::persist_apply_rename_pairs
│   │       ├── cleanup.rs               # 重复/不同版本检查；分块并行解析 manifest，两类扫描共用 GroupSpec trait + run_grouped_scan/preview_grouped；删除走 backup::prepare_mod_backup
│   │       ├── modify.rs                # 非纯 rename → op_pipeline::persist_apply_with_executor，备份对走 backup::prepare_mod_backup
│   │       └── scan.rs / zipmod.rs
│   ├── commands/                        # #[tauri::command]，纯转发
│   │   └── mod.rs / path.rs / dedup.rs / runtime.rs / move_file.rs /
│   │       preview.rs / settings.rs / records.rs /
│   │       suffix.rs / empty_dirs.rs / mod_tools.rs / pixiv_tag.rs
│   └── utils/
│       └── mod.rs / path.rs / hash.rs / filename.rs / time.rs
│           # filename: split_name_ext / resolve_conflict / resolve_conflict_with_reserved /
│           #          strip_conflict_suffix / normalize_suffix / extract_bracket /
│           #          sanitize_filename / normalize_brackets
│           # time:     system_time_to_secs（Metadata::modified/created → 秒级 Unix 时间戳）
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

- **commands**：解析参数、调 service、把 `AppError` 映射为 `String`。不写业务，不碰数据库。`settings::read_text_file` / `write_text_file` 是配置导入导出的通用 UTF-8 文本文件辅助命令，仅按用户通过系统对话框选择的路径读写内容。
- **services**：业务规则。涉及"记录+回滚"统一走 `op_pipeline`；涉及"事件推送"统一走 `events`。
- **db**：每个表/表组一个 repo；连接通过 `db::open_connection(db_path)` 按需打开（无连接池），统一把 SQLite 连接错误映射为 `AppError::Db`。"记录+items" 模式统一走 `op_record_repo`。

---

## 5. 核心抽象：op_record / op_pipeline / OpsPanel

后缀修改、空文件夹清理、Mod 重命名 / 归类 / 重复删除 / 旧版本删除 / 修改版本限制，本质都是："产生一批 `(old_path, new_path)` → 批量执行 → 写入可回滚记录"。项目把这一模式抽成三层，**任何新增此类业务都必须走这套抽象，严禁复制 suffix / mod_tools 当模板**。

### 5.1 后端

**`db::op_record_repo`**：通用 CRUD，通过 `OpRecordTables` 描述符传入表名与附加列（如 `target_suffix` / `kind`）。导出：
- `create_record(record_id, ..., rollback_enabled, ...)`：业务侧预生成 `record_id`（Mod 备份型操作要在 apply 前用它构造 `<backup_root>/<record_id>/...` 子目录），再传给 op_pipeline。`rollback_enabled = false` 的记录撤回时会被 `op_pipeline::rollback` 直接拒绝。
- `batch_insert_items` / `list_records(filter_extra_eq)` / `get_record_detail`
- `batch_update_rollback_results` / `set_record_rollback_status` / `delete_record` / `rename_record`

中性返回类型：`OpRecordSummary { record_id, record_name, extra, created_at, rollback_status, rollback_enabled, total_items, success_items }` / `OpRecordItem` / `OpRecordDetail`。**item 表结构硬约束**：`id / record_id / old_path / new_path / apply_success / apply_error / rollback_success / rollback_error / updated_at`——业务侧新增字段要另建辅助表。**主表硬约束**：`record_id / record_name / source_paths / created_at / rollback_status / rollback_enabled` 全部三类记录表都有这些列；业务侧的扩展列由 `OpRecordTables.extra_summary_column` 单独描述。

**`services::op_pipeline`**：
- `resolve_thread_count(db_path)`：从用户设置读有效线程数（0 → num_cpus）。**所有并发操作都走这里，严禁内联读 `settings.thread_count`**。
- `resolve_io_concurrency_multiplier(db_path)`：IO 并发倍率（默认 2，范围 1–16）。dedup 与 Mod 扫描的 Semaphore 许可 = `线程数 × 倍率`。SSD/NVMe 可上调到 4–8，HDD 压到 1。
- `resolve_text_preview_max_bytes(db_path)` / `resolve_zip_preview_max_entries(db_path)`：预览上限。
- `rayon_pool(thread_count)`：构造本次操作专用的 rayon 线程池，所有需要本地 rayon 池的业务都复用它，避免各模块重复 `ThreadPoolBuilder` 错误处理。
- `record_name_or_timestamp(record_name)`：记录名兜底统一为 `YYYY-MM-DD_HH-MM-SS`；哈希记录与记录型 apply 都走这里，避免多处硬编码格式串。
- `filter_by_selected_old_paths(items, selected_old_paths, get_old_path)`：apply 阶段把预览列表按用户勾选的 `selected_old_paths` 再筛一遍。`None` 视为全选；`Some(list)` 转 `HashSet` 后 retain。后缀修改 / Mod 重命名 / Mod 归类的 apply 都必须走这里，**禁止再写 inline 的 `let set = HashSet::from(...); preview.retain(...)`**。
- `rename_or_copy_delete(src, dst)`：rename 失败若是跨卷（Windows `ERROR_NOT_SAME_DEVICE` 17 / Unix `EXDEV` 18）退化为 `std::fs::copy` + `std::fs::remove_file`。`parallel_move`、`rollback`、去重后的普通文件移动、Pixiv 按 tag 移动都走它，所以目标目录 / 备份目录可跨盘配置。
- `parallel_move(pairs, create_parent, thread_count)`：并行 `rename_or_copy_delete`；`create_parent = true` 自动建目标父目录（归类需要）。
- `parallel_execute<F>(pairs, thread_count, executor)`：并行执行自定义闭包（如 modify 的"备份+重写 zip"或 cleanup 的"remove_file 不备份"）。
- `persist_apply_rename_pairs(record_id, rollback_enabled, ...)`：纯 rename 一步完成"建记录 + 并行 rename + 写 item"；调用方预生成 `record_id`。
- `persist_apply_with_executor(record_id, rollback_enabled, ..., executor)`：执行器自定义。
- `check_rollback(...)` / `rollback(..., force_ignore_missing)`：统一撤回实现；`rollback_enabled = false` 的记录直接报错拒绝撤回。

**撤回约定**：item 表记 `old_path` 与 `new_path`，默认 rollback = `rename_or_copy_delete(new_path → old_path)`。modify 业务把 `old_path = 原文件, new_path = 备份` 即可复用——apply 改写原文件、备份旁边放一份；rollback 自动把备份覆盖回原文件。空文件夹清理写 `old_path = new_path = 目录路径`，撤回由业务侧自定义为 `create_dir_all(old_path)`。Mod 备份型操作关闭回滚开关时 item.new_path 写空字符串、记录主表 `rollback_enabled = 0`；后端拒绝、前端按钮置灰。

**撤回的冲突保护**：rollback 是两阶段——先顺序用 `utils::filename::resolve_conflict_with_reserved` 给每条 item 解析最终目标（原路径已被外部新文件占用时自动加 ` (1)` / ` (2)` ... 后缀避让，预留集合保证同批次并行也不会重复分配 `(1)`），再并行 rename。冲突时仍记 `apply_success = true`，"已恢复到 X" 备注写进 `rollback_error` 列（该列既显示错误也显示备注）。这样用户在 apply 之后又往原目录放了同名文件、再点撤回的，**已存在的文件不会被静默覆盖**——撤回回来的文件会落到 `<原文件名> (N).<ext>`。空文件夹清理走自己的 rollback 实现，不参与此冲突解析（目录路径被非目录文件占用时直接报错）。

**Mod 备份型操作的标准前置**：`mod_tools::backup::prepare_mod_backup(db_path, selected_file_paths) -> PreparedBackup { rollback_enabled, record_id, pairs }` 是 cleanup / modify 公用入口，把"读 settings → 生成 record_id → 构造 `(原路径, 备份路径)` → 同批次同名撞名用 `resolve_conflict_with_reserved` 兜底"打成一个调用。`rollback_enabled = false` 时 pairs 的 new_path 全是空串，executor 据此切换到"真删 / in-place 改写"分支。**新增 Mod 备份型业务必须复用这个入口**，不要回到各自手抄 settings 读取 / Uuid 生成 / reserved 集合维护那套，否则只要逻辑漂移一处下次踩坑得在两份代码里同步修。

**模型层复用**：`SuffixApplyItem` / `EmptyDirApplyItem` 都为 `From<ModOpApplyItem>` 提供了 impl —— 业务侧把 `op_pipeline` 返回的 `ModOpApplyResponse.items` 直接 `.into_iter().map(SuffixApplyItem::from)` 转换即可，**不要在 service 里再写逐字段克隆的小函数**（`to_apply_item` / 闭包重映射）。结构后续若漂移也会被编译器立刻拦下来。

### 5.2 前端

**`types/opRecord.ts`** 提供 `OpApplyItem` / `OpApplyResponse` / `OpRecordSummary<Extra>` / `OpRecordItem` / `OpRecordDetail<Extra>` / `OpRollbackCheck` / `OpRollbackResponse`。基础类型都包含 `rollbackEnabled: boolean`，业务可继续用泛型扩展（如 `SuffixRecordSummary = OpRecordSummary<{ targetSuffix }>`、`ModOpRecordSummary = OpRecordSummary<{ kind: ... }>`）。

**`stores/_opRecordCrud.ts`** 提供 `createOpRecordCrudActions` / `createOpRecordCrudActionsWithRename` 两个 action 工厂，统一生成记录型 store 的 `refreshRecords / loadDetail / checkRollback / rollback / remove / removeBatch`，以及可选 `rename`。`suffix.ts` / `emptyDirs.ts` / `modTools.ts` 都通过 spread 工厂拿这些 CRUD action，只保留 preview/apply/scan 等业务专属逻辑。新增记录型业务时不要再手抄这些 action；如果该记录支持重命名，用 `createOpRecordCrudActionsWithRename`，否则用 `createOpRecordCrudActions`。

**`components/common/OpsPanel.vue`** 泛型面板，props：
- `paths` / `ensureNormalizedPaths`：路径规范化
- `columns` / `rows` / `rowKey`：VirtualTable 配置
- `preview` / `apply` / `checkRollback` / `rollback`：四个回调
- `applyItems` / `lastRecordId`：用于"撤回选中"的 itemId 映射
- `applyConfirmText` / `applyButtonText` / `previewToastBuilder` / `applySelectionFilter` / `infoTip`：UI 定制
- `#topForm` 插槽放业务专属表单（如"目标后缀"输入）

`SuffixPanel` / `ModRenamePanel` / `ModOrganizePanel` / `EmptyDirsPanel` 都是 **< 100 行** 的薄包装，只定义列 + rows computed + 四个回调。

**`components/RecordTab.vue`** 是记录管理页的通用 tab：负责搜索、批量删除、VirtualTable 列表、详情抽屉、撤回按钮与缺失路径确认；业务差异通过 `listColumns / detailColumns / searchableFields / extraFilter / #extraDescription / #rowActions` 注入。`RecordManagePage.vue` 中后缀 / 空文件夹 / Mod 三类记录都必须走 RecordTab；哈希记录因有"应用记录"与独立详情结构仍保留独立模板。

**`components/ModGroupPanel.vue`** 是重复 MOD / 不同版本 MOD 的共用分组检查面板。`ModDuplicatePanel.vue` 与 `ModVersionPanel.vue` 只做 `kind="duplicate" | "version"` 的薄包装；保留按钮、collapse 分组、删除选中、撤回本次等交互都在 ModGroupPanel 中维护。新增类似"按某个 manifest 维度分组 → 删除选中 → 写 Mod 记录"的面板时优先扩展 ModGroupPanel，不要复制两份分组表模板。

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
| `pixiv_tag_partial` | `PixivTagPartialPayload` | Pixiv tag 拉取增量结果（每批若干 PID 的 tags / error）|

事件名字面量统一在 [src-tauri/src/constants.rs](src-tauri/src/constants.rs) `events` 模块与前端 service 内，禁止硬编码。

### 命令清单（按 `lib.rs::invoke_handler!` 注册顺序）

- **路径**：`normalize_input_paths` / `reveal_in_explorer`
- **去重 / 运行时 / 移动**：`start_dedup_task` / `pause_task` / `resume_task` / `stop_task` / `get_move_summary` / `apply_move_action`
- **空文件夹清理**：`preview_empty_dirs` / `apply_empty_dir_cleanup` / `list_empty_dir_records` / `get_empty_dir_record_detail` / `check_empty_dir_rollback` / `rollback_empty_dir_cleanup` / `delete_empty_dir_record`
- **预览**：`request_preview`
- **设置 / 数据库**：`get_settings` / `save_settings` / `set_theme_mode` / `get_db_info` / `set_custom_db_path` / `delete_database` / `get_cpu_count` / `read_text_file` / `write_text_file`
- **哈希记录**：`list_hash_records` / `load_hash_record` / `rename_hash_record` / `delete_hash_record`
- **后缀修改**：`preview_suffix_change` / `apply_suffix_change` / `list_suffix_change_records` / `get_suffix_change_record_detail` / `check_suffix_rollback` / `delete_suffix_change_record` / `rollback_suffix_change`
- **Mod 工具**：`preview_mod_rename` / `apply_mod_rename` / `preview_mod_organize` / `apply_mod_organize` / `preview_mod_duplicates` / `start_mod_duplicate_task` / `apply_mod_duplicate_delete` / `preview_mod_versions` / `start_mod_version_task` / `apply_mod_version_delete` / `apply_mod_modify_version` / `list_mod_op_records` / `get_mod_op_record_detail` / `check_mod_op_rollback` / `rollback_mod_op` / `delete_mod_op_record` / `rename_mod_op_record` / `start_mod_scan_task`
- **Pixiv 标签整理**：`scan_pixiv_image_candidates`（同步）/ `start_pixiv_tag_scan_task`（长任务，入参 `pids` 而不是路径）/ `fetch_pixiv_tag_single`（重试）/ `move_image_by_tag_command`（移动）

### 长任务 task_id 约定

`start_dedup_task`、`start_pixiv_tag_scan_task` 与 Mod 各类长任务（`start_mod_scan_task` / `start_mod_duplicate_task` / `start_mod_version_task`）都接受可选 `taskId`——前端预生成 ID 并先开始监听，避免事件早于监听器到达造成丢失。前端本地 ID 统一用 `utils/taskId.ts::createLocalTaskId`；Mod / Pixiv 这类"前端预生成 ID → 后端事件终态"的长任务启动统一走 `composables/useLocalLongTask.ts::runLocalLongTask`，不要在 store 里再手写 taskId / runtime log / 失败兜底三件套。命令层统一用 `AppState::create_task(taskId)` 创建并注册 `(task_id, TaskRuntime)`；终态收尾统一用 `AppState::remove_task(task_id)`，不要在命令 / service 里直接操作 `state.tasks` 的锁。暂停 / 恢复 / 取消统一走 `TaskRuntime::pause` / `resume` / `cancel`，任务循环只读 `is_paused` / `is_cancelled`。去重结果缓存也由 `AppState::set_task_results` / `update_task_results` / `clear_task_results` 管理，外部不直接锁 `task_results`。Mod 同步命令（重命名 / 归类 / modify）也接受可选 `taskId` 用来发 `task_log`。

### 重复 / 版本检查的扫描流程

只走一次 WalkDir：先收集候选 PathBuf 列表，用 `len()` 作为进度 total；第二阶段分块（chunk = 256）并行解析 manifest，每个 chunk 处理完通过 `mod_duplicate_partial` / `mod_version_partial` 增量下发，避免一次性大响应。两类扫描的差异（分组 key、"成立"判定、partial event 类型、完成日志文案）抽成 `GroupSpec` trait（`DuplicateSpec` / `VersionSpec`），同步预览（`preview_grouped`）和长任务（`run_grouped_scan`）都共享同一份骨架——添加新分组维度时实现 trait 即可，不再复制 200+ 行扫描循环。前端展示对应收敛到 `ModGroupPanel.vue`，`ModDuplicatePanel.vue` / `ModVersionPanel.vue` 只是 `kind` 薄包装；添加新分组维度时优先扩展这套 kind 配置，不再复制 collapse + el-table 面板模板。`apply_mod_delete`（重复 / 版本删除）日志输出走"一行总结 + 抽样 5 条 + 余 N 条略"模板，避免 1w 选择刷屏；详情仍可在记录详情页查看。

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
- **文件名工具**：`split_name_ext` / `resolve_conflict` / `resolve_conflict_with_reserved` / `strip_conflict_suffix` / `normalize_suffix` / `extract_bracket` 等都在 [utils/filename.rs](src-tauri/src/utils/filename.rs)，禁止业务模块内联同名函数。**只要构造一批 `(old, new)` 路径**（preview / 备份目录映射 / 撤回目标解析），都必须用 `resolve_conflict_with_reserved` + `HashSet` 一次走完——`resolve_conflict` 只查磁盘，无法处理同批次多个不同源文件解析到同一目标的情况，会导致后续 rename 互相覆盖。当前已踩此坑的有：suffix preview（`foo.tmp` 与 `foo.bak` 都改成 `foo.txt`）、organize preview（多个源目录的 `[X]foo.zipmod` 归到同一 `[X]/`）、cleanup/modify 的备份路径（多个源目录同名 zipmod 落入同一 `<backup_root>/<record_id>/`）、op_pipeline::rollback（外部新文件占用原路径）。
- **并发**：线程数统一 `op_pipeline::resolve_thread_count(db_path)`；自定义线程池用 `op_pipeline::rayon_pool(thread_count)?.install()` 本地化，不碰全局池。
- **数据库迁移**：新增表用 `CREATE TABLE IF NOT EXISTS`；新增列 `let _ = conn.execute("ALTER TABLE ... ADD COLUMN ...")` 忽略重复列错误。不改已有列。
- **数据库连接**：repo / schema 里按需调用 `db::open_connection(db_path)`，不要重新写 `Connection::open(...).map_err(...)` 的局部 helper。
- **长任务运行时**：命令层创建长任务用 `AppState::create_task`，终态清理用 `AppState::remove_task`；运行控制用 `TaskRuntime::{pause,resume,cancel}`，去重结果缓存用 `AppState::{set_task_results,update_task_results,clear_task_results}`，不要在命令 / service 里直接写 `paused` / `cancelled` 原子位或手动遍历 / 加锁 `AppState` 内部表。`AppState::has_active_tasks()` 判定基于"tasks 表是否非空"——只要任务还没走完终态收尾（包括取消后的备份目录 rename / 哈希记录落盘等），就视为活跃。`delete_database` 等需要文件独占的操作必须走这条更严格的判定，否则取消瞬间 + 删库会与正在 commit 的事务竞争。
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

虚拟滚动表格,默认固定行高,可选**行高内容自适应**;支持列拖拽、全选、客户端/服务端分页、固定列(sticky)、ellipsis + tooltip、空态占位、**列自定义(显示/左固定/顺序)**、双击复制单元格文本。列定义类型 `VirtualColumn`,运行时配置类型 `VirtualColumnState`。列宽拖拽 `requestAnimationFrame` 节流。

**滚动条不闪烁的关键：`scrollbar-gutter: stable`**。`.vtable-scroll` 永远预留 ~15px 纵向滚动条宽度，让 `clientWidth` 在内容行变多/变少时不再震荡；否则 fit-width 模式下纵向条出现 → clientWidth 缩小 → 列宽重算 → 内容收住 → 纵向条消失 → clientWidth 复位 → 反复闪烁。fit-width 启用且**列总最小宽度 ≤ 容器宽度**时强制 `overflow-x: hidden` 兜底 1px 舍入；列总最小宽度撑出容器（窄屏 + 多列）时 `fitWidthApplied` 退化为 false、`--fit` 不再附加，让 `overflow-x: auto` 接管显示横向滚动条 —— 用户能滑看被挤出去的列而不是被一刀切。ResizeObserver 回调 rAF 合并多次 entry。

**横向滚动条不引发不必要纵向滚动条**：`fitWidthApplied = false` 时横向滚动条占据底部 ~17px，`clientHeight` 缩水，本来"刚好装下"的内容会被挤压触发**不必要的纵向滚动条**（容器实高 400 → 无横滚 clientHeight 400、有横滚 383；HEADER 36 + 内容 360 = 396 → 浏览器误判需要纵向条）。`verticallyOverflowing` computed 在 JS 侧加回 `HORIZONTAL_SCROLLBAR_ALLOWANCE = 17`（fit-width 生效时为 0），判定不需要纵向滚动时通过 `.vtable-scroll--no-vscroll` 强制 `overflow-y: hidden` 覆盖默认 auto；内容真的多到溢出时本类不应用，纵向条照常出。

**高度两种模式**：
- 固定高度：传 `:height="<px>"`。仅当父容器无固定高度又必须确定尺寸时使用；目前只有 `MoveReportDialog`（el-dialog 按内容撑高）。
- 自适应：省略 `height`，`.vtable` 与 `.vtable-scroll` 都用 `flex: 1; min-height: 0`。**主页面 / 详情抽屉 / OpsPanel / ModScanPanel 统一此模式**。硬要求：**父链路径一路 `display: flex; flex-direction: column; min-height: 0`**——任何一层断 flex 或少 `min-height: 0` 都会塌缩或溢出多余滚动条。不要写 `tableHeight = panelHeight - 180` 这种魔数。`el-drawer__body` 等需要 `:deep()` 改成 flex column，参见 `RecordManagePage.vue` / `RecordDetailDrawer.vue`。

**列宽自适应（`fit-width`）**：开启后总列宽永远等于容器宽度，消除横向滚动。算法用 `width - minWidth` 作为伸缩权重：只声明 `width` 视为固定列（权重 0），只声明 `minWidth` 是弹性列。**所有列 `minWidth` 之和大于容器宽时 `fitWidthApplied` 退化为 false**：列宽回退到原始 `colWidths`、外层 `--fit` class 同步移除，`overflow-x: auto` 接管让横向滚动条出现，用户能滑动浏览被挤出去的列。OpsPanel / ModScanPanel 默认开启；不传 `fit-width` 则保持原行为（如 RecordManagePage 主表）。

**行高自适应（`auto-row-height`）**：默认关闭;开启后行高由 cell 内容实测决定,而不是固定 `itemHeight`。实现用 `ResizeObserver` 观察每个 `.row` 元素,测得高度按 rowKey 写进 `measuredRowHeights` Map,rAF 批量提交;再用前缀和 `rowOffsets` 给每行算 `top`,可见区间用二分查找定位。未测量的行用 `itemHeight` 作为预估高度——所以开启此模式后 `itemHeight` 的语义从"固定行高"变成"行预估高度",传一个接近典型行的值能减少滚动条抖动。**只在每行内容高度差异巨大、单元格内部又不能加滚动条的场景用**(目前只有 PixivTagPanel 的 tag 气泡墙列);常规场景仍走默认固定行高,数学最快也最稳。

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
- `columns` 运行时变更自动 reconcile：被删除列剔除；新列**按它在 `props.columns` 里的源索引插入到现有 saved order 的合适位置**——找第一条"源索引比新列大"的已存在列，插它前面；找不到就 push 到末尾。这样调用方在 props 里把新列放在第 1 位（如 PixivTagPanel 开启缩略图后图片列），用户已有持久化顺序仍能让新列出现在最左边而不是末尾。reconcile 完成后 `normalizeFixedContiguity` 兜底固定连续性。
- 关闭功能退回旧版表头：传 `:column-configurable="false"`。
- `slotRefreshKey`：当具名 slot 的内容依赖 `rows` / `columns` 之外的响应式状态时传入轻量 revision；VirtualTable 会把它拼进具名 slot 单元格的 vnode key，使可见行 slot 在 revision 变化时重建。PixivTagPanel 用它确保全局译名开关 / 排除 tag / 本地 tag 翻译改动后 chip 文本立刻从原 tag 切到本地译名 / `translation.en` 或反向切回。

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

Mod 各面板（Rename / Organize / Duplicate / Version / Scan）与去重分组表的路径列统一使用 `PathPreviewLink`：组件内部负责去掉 Windows 扩展前缀、包一层 `PreviewPanel` 提供 hover 预览，并通过 `revealInExplorer(path)` 点击定位。需要只显示文件名时传 `label`，已有常驻缩略图等不需要 hover 预览的场景传 `:preview="false"`。`.path-link` 样式只保留在 `PathPreviewLink.vue` 内，业务面板不要重复定义。**不再单独保留"目录"操作列**。

---

## 9. 性能与并发约定

- **高频日志事件**：每条 `task_log` 是单条 IPC。前端 `runtime` store 把事件先写非响应式缓冲，每 150ms 批量刷入响应式 `logs`，超过 `LOG_MAX_LENGTH = 3000` 裁剪旧数据。后端如需进一步合批要新增 `task_log_batch` 事件并改前端监听器（目前未做）；热路径请只在 chunk / 阶段边界打日志。`PARTIAL_BATCH_SIZE = 30` 仅用于去重 `task_result_partial`。
- **去重流水线**：扫描 → 哈希（Semaphore 限流，许可 = 线程数 × 倍率）→ 分组 → 发送。流式 `mpsc` 通道边收边分组。失败状态（扫描 future panic / 内部 IO 错误）由命令层统一发 `task_state_changed=Failed`，service 内部只 return `Err`，不再自行 emit——避免双重 Failed 事件。取消时同步 `Cancelled` 状态后立刻返回，半截分组缓存不会写进 `set_task_results`，下一次"重做"看到的是空缓存而不是上次取消瞬间的快照。
- **Mod 扫描流水线**：tokio Semaphore 并发 = `线程数 × 倍率`；zip 读取 `tokio::task::spawn_blocking`；匹配结果进 `Arc<Mutex<Vec<_>>>`。
- **重复 / 不同版本检查流水线**：单次 WalkDir 长任务。第一遍只收候选 PathBuf 列表（用 `len()` 当 total），第二阶段固定 chunk = 256 用 `op_pipeline::rayon_pool` 本地线程池并行解析 manifest，每 chunk 完成立刻聚合 + 增量推送（`mod_duplicate_partial` / `mod_version_partial`），避免一次性大响应。
- **modify 流水线**：rayon 并行；每个文件 copy → 重写 zip（`raw_copy_file` 零重编码复制非 manifest 条目）→ atomic rename。失败自动清理临时文件 + 备份。
- **暂停/取消**：`TaskRuntime` 内部用原子位记录 paused / cancelled；外部通过 `pause` / `resume` / `cancel` 写入，通过 `is_paused` / `is_cancelled` 读取。扫描阶段只响应取消，哈希调度阶段同时响应取消与暂停；已提交的哈希任务跑完。
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
3. **记录型操作**：后端走 `op_record_repo + op_pipeline`，任务面板走 `OpsPanel` 薄包装，记录管理页走 `RecordTab`，前端 store 用 `stores/_opRecordCrud.ts` 的工厂生成通用 CRUD action。非纯 rename 用 `persist_apply_with_executor`，item 记 `old_path = 原始, new_path = 备份`。
4. **数据库迁移**：只允许 `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE ... ADD COLUMN`，不改已有列。迁移写在 `schema.rs::init_schema` 末尾。
5. **IPC 事件**：新事件登记 `constants.rs::events`，写 `services/events.rs` emit 函数，在前端 store 的 `initEvents` 监听。长任务 spawn 失败兜底统一调 `events::finalize_failed_long_task(app, state, task_id, err)`：发 `task_state_changed=Failed` + `task_failed` 事件 + `state.remove_task`。partial done 信号需要业务侧自行先发（不同任务 payload 不同），再调本 helper。**禁止在每个命令模块再抄一份 emit_state_changed + emit_task_failed + remove_task 三件套**。命令层启动长任务统一走 `events::spawn_long_task(app, state, task_id, make_future, on_failure_extra)`：克隆 `state/app/task_id` 给 spawn 闭包、把业务 future 喂进去、失败时先调 `on_failure_extra`（用来发本业务的 partial done）再走 `finalize_failed_long_task`。dedup / mod_scan / mod_duplicate / mod_version 四个长任务都走这套；pixiv 是例外——它内部已经统一终态收尾（`finalize_done`/`finalize_failed`），命令层只 spawn 不做兜底，否则失败会发两次 `Failed`。
6. **路径**：`std::fs` 入参一律 `to_extended_length_path`；返回前端的路径一律 `to_user_friendly_path`。
7. **并发**：线程数从 `op_pipeline::resolve_thread_count` 取，不要 inline 读 `settings.thread_count`。
8. **文件名**：用 `utils::filename` 里的函数，不要自己实现一份。
9. **注释**：新的 `pub fn` / `pub struct` / `pub enum` 必须 `///`；新的 TS `export function` / store action 必须 JSDoc。
10. **验证**：`cargo check`（src-tauri/）+ `npx vue-tsc --noEmit`（根目录）双把关；UI 流程跑 `npm run tauri dev`。
11. **同步本文档**：按 §0 把本轮改动反映到对应段落。

---

## 附录：常量速查

### 事件 / 枚举（硬编码，不开放给用户）

- 事件：`constants::events::{TASK_LOG, TASK_PROGRESS, TASK_STATE_CHANGED, TASK_FAILED, TASK_RESULT_PARTIAL, TASK_COMPLETED, MOVE_REPORT_READY, MOD_SCAN_COMPLETED, MOD_DUPLICATE_PARTIAL, MOD_VERSION_PARTIAL, PIXIV_TAG_PARTIAL}`
- 阶段：`constants::stages::{SCAN, HASH, MOD_SCAN, MOD_DUPLICATE, MOD_VERSION, PIXIV_TAG}`
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
| `mod_rollback_enabled` | bool | true | 是否为 Mod 备份型操作（重复删除 / 不同版本删除 / 移除版本限制）创建备份。关闭后这三类不留痕、记录 `rollback_enabled = 0`、撤回按钮置灰。重命名 / 归类不受影响。 |
| `mod_backup_dir` | str? | null | Mod 备份根目录；为空时使用 `<exe_dir>/mod-backups`。每条记录落 `<root>/<record_id>/<原文件名>`，跨卷场景由 `op_pipeline::rename_or_copy_delete` 用 copy + delete 兜底。 |
| `pixiv_tag_api_base` | str | `https://www.pixiv.net/ajax/illust/` | Pixiv tag 接口 base URL；最终请求 = base + PID（base 末尾是否带斜杠都接受）。 |
| `pixiv_excluded_tags` | str[] | `[]` | 不渲染为 chip 的 tag；落库为 JSON 数组字符串。排除判断同时匹配原 tag 与当前显示文本。配置中心支持导入 JSON 数组 / `{ tags: [] }` / 文本分隔列表，导出为 JSON。 |
| `pixiv_local_tag_translations` | object | `{}` | 本地 tag 翻译表，key 为 Pixiv 原 tag，value 为本地译名；落库为 JSON 对象字符串。开启 `pixiv_use_translation` 或单行强制译名时，本地译名优先于 Pixiv 响应里的 `translation.en`。配置中心支持导入 JSON 对象 / `{ translations: {} }` / `原tag=译名` 文本，导出为 JSON。 |
| `pixiv_cookie` | str? | null | Pixiv Cookie；填了能拿到 R-18 / 关注限定 tag。仅本机保存，不上传。 |
| `pixiv_proxy` | str? | null | Pixiv 接口的 HTTP / HTTPS / SOCKS5 代理 URL（如 `http://127.0.0.1:7890`、`socks5://127.0.0.1:1080`）。中国大陆访问 Pixiv 一般要配。留空则按 reqwest 默认走 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量。 |
| `pixiv_use_translation` | bool | false | 是否在 chip 上用译名替代原 tag 显示。开启时点击移动也按译名建子目录；本地翻译优先，其次 Pixiv 响应里的 `translation.en`，缺译名的 tag 自动回落原 tag。任务面板顶部"使用英文译名"开关与本设置同步。 |
| `pixiv_rate_limit_per_minute` | i32 | 60 | Pixiv 拉取的每分钟最大请求数。0 视为不限速，UI 限制最小 1、最大 600。所有并发 worker 与单条重试共享同一条 next-slot 节流队列，整体速率被钉死在 `值/60` 次/秒——并发只控"同时在飞的请求数"，本设置控"任意 60s 滚动窗口内总请求数"。 |
| `pixiv_partial_flush_interval_ms` | i32 | 0 | Pixiv 增量结果在前端的合并刷新间隔（毫秒）。0 = 实时（partial 一到达就 commit）；>0 = 节流到固定间隔（多个 partial 合并到一次 commit）。UI 范围 0–10000ms。`done` 终态会立刻 flush，不被节流拖延。50K 张图配 300–800ms 节流明显降低 UI 抖动，不影响后端拉取速度。 |

新增配置项的步骤：`models/settings.rs` 加字段 + 默认 → `db/schema.rs` 末尾 `ALTER TABLE ADD COLUMN` → `db/settings_repo.rs` 的 SELECT/UPDATE 扩列 → `types/settings.ts` 加字段 → `stores/config.ts` 初始 state 加默认 → `views/SettingsPage.vue` 加表单项。`DEFAULT_*` 兜底常量集中在 [src-tauri/src/config.rs](src-tauri/src/config.rs)。

### 非配置常量（编译期硬编码）

- 前端：`src/constants/app.ts`（`DEFAULT_LOG_MAX_LENGTH` 兜底、`LOG_FLUSH_INTERVAL`）、`task.ts`（`DEFAULT_EXTREME_ROW_THRESHOLD` 兜底 / `EXTREME_OVERSCAN` / `NORMAL_OVERSCAN` / 分组分页与渲染步长）、`theme.ts`、`preview.ts`、`recordColumns.ts`（记录管理页列定义与 kind 文案）。
- 后端：`src-tauri/src/config.rs`（`HASH_QUEUE_SIZE` / `PARTIAL_BATCH_SIZE` / `PAUSE_SLEEP_MS`，加一组 `DEFAULT_*` 作为配置兜底）。
