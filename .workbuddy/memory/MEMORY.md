# 项目长期约定

## 短语库数据模型（已落地到 db 结构）
- 选型：单 db 单表 `phrase_table` + 新列 `source`('base'|'user') + `state`(INTEGER, **两态: 0 显示 | 1 隐藏**；用户 2026-10-26 拍板：覆盖/删除 base 都归为"隐藏"，弃用 -1)。**已去掉 `UNIQUE(keyboard)` 约束**（2026-10-26 用户决定：撞关键词时保留 base 原行 + 另建合并 user 行，允许同 keyword 双行；**db 列名已统一为 `keyword`**，2026-10-26 由 keyboard 重命名而来）。
- 铁律：base 只读；**同 keyword 可并存 base 行 + user 行**（双行模型）；user 行永远是 state=0（始终显示）；**base 行 state=0 显示、state=1 隐藏**（被 user 覆盖或用户删除都标 1）；覆盖/合并 base=另建或更新 user 行（list=base.list+user.list 去重，或用户完整内容）并把对应 base 行 state 设 1（base 的 list 不动）；删 user=真删 DELETE（若删的是覆盖类 user 行，base 行不动、保持 state=1 隐藏，该 keyword 整体不再显示，不回 0）；升级 base=DELETE WHERE source='base' 再重导，user 行不动；导出/互传只抽 source='user'。**无独立"还原 base"操作——已覆盖的 base 删除其 user 行后即保持 state=1 隐藏，若要恢复显示只能重导 base（会清掉所有 user 覆盖，需谨慎）。**
- 读取/展示约定（app 代码尚未实现，仅 db 层面）：同 keyword 有 user 行时优先显示 user 行、隐藏 base 行。查询骨架：`SELECT ... WHERE source='user' UNION ALL SELECT ... FROM phrase_table b WHERE b.source='base' AND b.state=0 AND NOT EXISTS(SELECT 1 FROM phrase_table u WHERE u.source='user' AND u.keyboard=b.keyboard)`（base 的 state=1 即隐藏、不出现）。
- **已执行**：2026-10-26 把 release phrase_db.db 里 67 个被 user 覆盖的 base 行 `UPDATE state=1`（依据 `WHERE source='base' AND keyboard IN (SELECT keyboard FROM phrase_table WHERE source='user')`）。现 base 分布 580×state0 + 67×state1，user 120×state0。
- state 两态只治理 base 行；user 行恒定 state=0（底下无 base 可覆盖/隐藏）。
- 种子源文件（规范化 JSON 源）：`F:/html/tauri-app/base_phrase.json`（**当前 9079 组**，由 CET-4 手写种子 + 用户 `englishPhrases.db`(8440 条) 合并而来；字段 type/first_char/keyword(=db的keyboard)/list；顶层 source='base'；_template 示范键导入忽略）。该文件 2026-11-06 曾丢失，已从合并后的 seed db 用 gen_seed.py 思路重建，与 db 完全一致。
- 旧示例种子：`F:/html/tauri-app/base_phrases_seed.json`（15条小样例）。
- 落地涉及：phrase.rs / import.rs / export.rs / send.rs / 前端 PhrasePage.js。**已落地**：phrase.rs 的 `load_phrases_page` 与 `search_phrases` 已改双行 UNION 查询（user 行全出 + base 行仅 `state=0` 且无同 keyboard 的 user 行才出），`CREATE TABLE` 已补 `source`/`state` 列带默认值（source 默认 'user'、state 默认 0）。**未改（待办）**：`save_phrase`（按 keyboard 判重会挡住覆盖 base 的编辑/新增）、`delete_phrase`（按 keyword 删除会同时删 base+user 两行，违背"删 user 不动 base"模型）需按 source/state 路由；前端 PhrasePage.js 编辑/删除路由待接 source。

## 关键数据位置（重要！）
- app 的短语 db 路径由 `english/phrase.rs` 的 `get_db_path` 决定：`exe_dir/data/subject/english/phrase_db.db`（桌面 Windows）；debug 与 release 模式都拼 `data/subject/english`。
- 用户真实活数据在 `src-tauri/target/debug/data/subject/english/phrase_db.db`（40 行，含 15 原始 + 25 其他）；release 那个路径导入前是空的（0 行）。另有非 app 使用的 `src-tauri/target/release/data/english/phrase_db.db`（15 行，勿混淆）。
- 已用脚本把 base_phrase.json 的 647 条以 source='base',state=0 写入两个 `data/subject/english/phrase_db.db`（debug: 40 user+631 base；release: 647 base）。debug 库导入前有 .bak 备份。

## 打包与运行时路径约定（2026-10-27 定稿）
- **种子（只读模板）**：`src-tauri/resource/subject/english/phrase_db.db`，纯 base、state=0（**当前 9079 行**），由 `src-tauri/scripts/gen_seed.py` 从 `base_phrase.json` 生成。`tauri.conf.json` 的 `bundle.resources` 已声明 `{"resource": "resource"}`。`gen_seed.py` 在 2026-11-06 曾被确认丢失，已重建（读 base_phrase.json → 写 resource db，与运行时七列表结构一致）。
- **桌面运行时**（便携式，无安装包，用户自有打包程序把 resource 放 exe 旁）：
  - 工作库 `exe_dir/data/subject/english/phrase_db.db`（`get_data_root`=exe_dir/data）
  - `config.json` = `exe_dir/config.json`（`core/config.rs`，桌面取 exe 目录）
  - `init.json` = **`exe_dir/init.json`**（`core/init.rs`，2026-10-27 由 `exe_dir/data/` 移到 exe 同目录，与 config.json 同级）
  - 种子查找候选链：`exe_dir/resource/...` → `../../src-tauri/resource/...`（dev）
- **Android 运行时**：`app_config_dir()` 与 `app_data_dir()` 在 tauri 2.12 都解析为 `activity.dataDir` = `/data/data/<identifier>`（本项目 `com.jy.tauri-app`）。所以三者同层并列：
  - `config.json` = `/data/data/com.jy.tauri-app/config.json`
  - `init.json` = `/data/data/com.jy.tauri-app/init.json`（= app_data_dir 根）
  - 工作库 = `/data/data/com.jy.tauri-app/subject/english/phrase_db.db`
  - 种子在 APK 内 `assets`（`resource_dir()`），压缩无法直接被 SQLite 打开 → 先 copy 到 `app_data_dir/_seed_cache/` 再只读打开，用完清理。
- **初始化/迁移**：`core/init.rs` 的 `ensure_init_and_migrate`（在 `.setup` 里、HTTP 服务前调用）；`BASE_DB_VERSION` 硬编码，改基础词流程 = 改 JSON → 重跑 gen_seed.py → 版本 +1 → 重编。迁移单事务、幂等；工作库文件不存在时强制重迁。

## 导入脚本列名坑
- db 列名原为 `keyboard`，2026-10-26 已全局改名 `keyword`；历史脚本若还写 keyboard 会报 no such column。

## 导入完成提示口径（用户 2026-11-07 拍板，勿再收窄）
- `App.vue` 的 `importSuccessCount` = 第一阶段 `res.imported`（短语静默插入词条 + 笔记目录/笔记）+ 冲突弹窗里「追加/覆盖」裁决成功的条数（`resolve_duplicates_batch` 返回的 `resolved`，逐条队列与勾选批量两条路径都要累加）。
- **不单独显示「重复关键词条数」**，但被追加/覆盖的词确实落库必须计入；跳过、失败、取消不计入。
- 反例：`phrase_backup_20261105.zip`（121 短语 + 1 笔记目录）整包全撞词时，只算 `res.imported` 会显示「共导入 1 条」，正确应为 122 条。
