// ============================================================
// init.rs —— 首次启动环境初始化 + 基础词数据库迁移
// 设计（与用户确认，2026-10-27）：
//   · init.json 记录 base_db_version（“已应用”版本）与 migration_done
//     位置：桌面 = exe 同目录；移动 = app_data_dir（app 私有目录）
//   · 版本目标由 Rust 硬编码常量 BASE_DB_VERSION 决定（改基础词需重编）
//   · 首次启动：建 init.json（migration_done=false）→ 执行迁移
//   · 后续启动：仅比对版本；不一致 → migration_done=false → 执行迁移
//   · 迁移：把种子 db（resource/phrase_db.db，纯 base）合并进工作库
//       - base 行整体替换为种子内容（干净，移除的短语不残留）
//       - user 行仅追加种子短语（按 phrase 去重，绝不覆盖用户内容）
//       - base 状态：该关键词存在 user 行 → 1（隐藏）；否则 → 0
//       - 清理工作库中“已不在种子里”的 base 行（保持 base 干净）
//   · 安卓：种子在 APK 内（压缩）无法直接被 SQLite 打开，须先复制到
//     app_data_dir 再只读打开；桌面种子是普通文件，直接只读打开。
//   · 幂等 + 事务：崩溃后 migration_done 仍为 false，下次启动自动重试。
// ============================================================

use rusqlite::OptionalExtension;
use rusqlite::{Connection, OpenFlags, params};
use std::fs;
use std::path::{Path, PathBuf};

/// 基础词数据库版本（硬编码）。改基础词并重新生成种子 db 后，手动 +1 并重新编译。
pub const BASE_DB_VERSION: i32 = 2;

const INIT_FILE_NAME: &str = "init.json";

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct InitConfig {
    base_db_version: i32,
    migration_done: bool,
}

fn default_init() -> InitConfig {
    // 首次运行：默认版本=当前常量、未完成 → 触发迁移
    InitConfig {
        base_db_version: BASE_DB_VERSION,
        migration_done: false,
    }
}

/// 工作库根目录（与 get_db_path 共用）：桌面 = exe_dir/data；移动 = app_data_dir
fn data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    crate::english::phrase::get_data_root(app)
}

/// init.json 所在目录（与工作库分开，不放进 data 子目录）：
/// - 桌面：exe 同目录（便携式，随软件走，与 config.json 同级）
/// - 移动：app_data_dir（Android 安装包只读，只能写应用私有目录；
///   该目录下 config.json / init.json / subject/ 并列）
#[cfg(desktop)]
fn init_dir(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;
    Ok(exe_dir.to_path_buf())
}

#[cfg(mobile)]
fn init_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    data_root(app)
}

fn init_json_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(init_dir(app)?.join(INIT_FILE_NAME))
}

fn read_init(path: &Path) -> InitConfig {
    match fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|_| default_init()),
        Err(_) => default_init(),
    }
}

fn write_init(path: &Path, cfg: &InitConfig) -> Result<(), String> {
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

/// 定位种子 db（纯 base）。仅桌面使用：候选链 = 构建/便携（exe 旁 resource）
/// 与开发（target 内 exe → ../../src-tauri/resource）。移动端改用编译期嵌入字节（见下方 SEED_DB_BYTES）。
#[cfg(desktop)]
fn seed_db_path(_app: &tauri::AppHandle) -> PathBuf {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
    let cand = [
        exe_dir
            .join("resource")
            .join("subject")
            .join("english")
            .join("phrase_db.db"),
        exe_dir.join("../../src-tauri/resource/subject/english/phrase_db.db"),
    ];
    for c in &cand {
        if c.exists() {
            return c.clone();
        }
    }
    cand[0].clone() // 都不存在：返回首选，交由调用方告警
}

/// 安卓/iOS 种子 db 字节（编译期编入二进制，绕开 APK/IPA 内 assets 的虚拟 URI——
/// 移动端 resource_dir() 返回 asset://localhost/... 而非真实文件，std::fs 读不到）。
/// 改基础词流程：重新生成种子 db → 重新编译移动端。
#[cfg(mobile)]
const SEED_DB_BYTES: &[u8] = include_bytes!("../../resource/subject/english/phrase_db.db");

/// 打开种子（只读）。移动端：把编译期嵌入的种子字节写成可写缓存文件再只读打开
/// （绕开 asset URI，且 SQLite 需要真实文件）。返回 (连接, 临时缓存路径——用于事后清理)。
#[cfg(mobile)]
fn open_seed_readonly(app: &tauri::AppHandle) -> Result<(Connection, Option<PathBuf>), String> {
    let cache_dir = data_root(app)?.join("_seed_cache");
    fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let dst = cache_dir.join("phrase_db.db");
    fs::write(&dst, SEED_DB_BYTES).map_err(|e| e.to_string())?;
    let conn = Connection::open_with_flags(&dst, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    Ok((conn, Some(dst)))
}

#[cfg(desktop)]
fn open_seed_readonly(_app: &tauri::AppHandle) -> Result<(Connection, Option<PathBuf>), String> {
    let src = seed_db_path(_app);
    if !src.exists() {
        return Err("种子 db 不存在".into());
    }
    let conn = Connection::open_with_flags(&src, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    Ok((conn, None))
}

/// 执行一次迁移：在事务内把种子 base 合并进工作库（幂等）。
fn run_migration(app: &tauri::AppHandle) -> Result<(), String> {
    let working_path = crate::english::phrase::get_db_path(app)?;
    if let Some(parent) = working_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // 只读打开种子
    let (seed_conn, seed_cache) = open_seed_readonly(app)?;

    // 打开工作库并确保表结构
    let wconn = Connection::open(&working_path).map_err(|e| e.to_string())?;
    wconn
        .execute(
            "CREATE TABLE IF NOT EXISTS phrase_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT,
                first_char TEXT,
                keyword TEXT,
                list TEXT,
                source TEXT DEFAULT 'user',
                state INTEGER DEFAULT 0
            )",
            [],
        )
        .map_err(|e| e.to_string())?;

    // 迁移前先建索引：下面会对每个种子逐条做 3~5 次
    // WHERE source=? AND keyword=? 查询，无索引时是 O(N²) 全表扫（2890 条尤其明显）。
    // 索引挂在可写的工作库上，种子库只读、不改动。
    crate::english::phrase::ensure_phrase_indexes(&wconn);

    // 收集种子所有 base 行
    let mut seed_stmt = seed_conn
        .prepare("SELECT keyword, type, first_char, list FROM phrase_table WHERE source='base'")
        .map_err(|e| e.to_string())?;
    let seed_rows: Vec<(String, String, String, String)> = seed_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let seed_keywords: Vec<String> = seed_rows.iter().map(|r| r.0.clone()).collect();

    wconn.execute("BEGIN", []).map_err(|e| e.to_string())?;

    let result: Result<(), String> = (|| {
        for (kw, ty, fc, seed_list) in &seed_rows {
            // 该关键词在工作库中是否已有 user 行（用户覆盖/自定义）
            let has_user: bool = wconn
                .query_row(
                    "SELECT 1 FROM phrase_table WHERE source='user' AND keyword=?1 LIMIT 1",
                    params![kw],
                    |_| Ok(true),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .is_some();

            if has_user {
                // 把种子短语追加进 user 行（按 phrase 去重，绝不覆盖用户已有内容）
                let user_list_text: String = wconn
                    .query_row(
                        "SELECT list FROM phrase_table WHERE source='user' AND keyword=?1 LIMIT 1",
                        params![kw],
                        |row| row.get::<_, String>(0),
                    )
                    .map_err(|e| e.to_string())?;
                let mut user_list: Vec<serde_json::Value> =
                    serde_json::from_str(&user_list_text).unwrap_or_default();
                let seed_list_val: Vec<serde_json::Value> =
                    serde_json::from_str(seed_list).unwrap_or_default();
                for item in seed_list_val {
                    let phrase = item.get("phrase").and_then(|v| v.as_str()).unwrap_or("");
                    let exists = user_list
                        .iter()
                        .any(|u| u.get("phrase").and_then(|v| v.as_str()).unwrap_or("") == phrase);
                    if !phrase.is_empty() && !exists {
                        user_list.push(item);
                    }
                }
                let new_user_list = serde_json::to_string(&user_list).map_err(|e| e.to_string())?;
                wconn
                    .execute(
                        "UPDATE phrase_table SET list=?1 WHERE source='user' AND keyword=?2",
                        params![new_user_list, kw],
                    )
                    .map_err(|e| e.to_string())?;
            }

            // base 行合并规则：state=1 是【终态】——一旦被用户覆盖/删除而屏蔽，
            // 之后不再被任何逻辑改动（不刷内容、不重置 state、不在下方清理里删除）。
            // 否则每次基础库升级都会把用户删掉的词重新写回 state=0「复活」。
            // ORDER BY state ASC：同 keyword 若历史库里同时存在 0/1 两行，
            // 只要还有一行 state=0 就按「有可见行」处理，UPDATE 也只命中 state=0。
            let base_state: Option<i64> = wconn
                .query_row(
                    "SELECT COALESCE(state,0) FROM phrase_table WHERE source='base' AND keyword=?1 \
                     ORDER BY COALESCE(state,0) ASC LIMIT 1",
                    params![kw],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;

            match base_state {
                Some(1) => continue, // 全是已屏蔽行：整条跳过，也不许 INSERT 出新的可见 base
                Some(_) => {
                    // 仅更新未被屏蔽的 base 行；有 user 覆盖行时同步标 1
                    let state = if has_user { 1 } else { 0 };
                    wconn
                        .execute(
                            "UPDATE phrase_table SET type=?1, first_char=?2, list=?3, state=?4 \
                             WHERE source='base' AND keyword=?5 AND state=0",
                            params![ty, fc, seed_list, state, kw],
                        )
                        .map_err(|e| e.to_string())?;
                }
                None => {
                    // 工作库里从未有过该 base 行：按种子新建
                    let state = if has_user { 1 } else { 0 };
                    wconn
                        .execute(
                            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
                             VALUES (?1, ?2, ?3, ?4, 'base', ?5)",
                            params![ty, fc, kw, seed_list, state],
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        // 清理工作库中“已不在种子里”的 base 行（保持 base 干净）。
        // 只删 state=0 的：已屏蔽的 base 是终态，一律原地保留不动。
        if !seed_keywords.is_empty() {
            let placeholders: String = seed_keywords
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "DELETE FROM phrase_table WHERE source='base' AND state=0 AND keyword NOT IN ({})",
                placeholders
            );
            let params = rusqlite::params_from_iter(seed_keywords.iter().map(|s| s.as_str()));
            wconn.execute(&sql, params).map_err(|e| e.to_string())?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            wconn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            cleanup_seed_cache(seed_cache);
            Ok(())
        }
        Err(e) => {
            let _ = wconn.execute("ROLLBACK", []);
            cleanup_seed_cache(seed_cache);
            Err(e)
        }
    }
}

fn cleanup_seed_cache(seed_cache: Option<PathBuf>) {
    if let Some(dst) = seed_cache {
        let _ = fs::remove_file(&dst);
        if let Some(parent) = dst.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

/// 启动入口：核对 init.json，必要时执行迁移。失败（如种子缺失）仅告警，不阻断 App 启动。
pub fn ensure_init_and_migrate(app: &tauri::AppHandle) -> Result<(), String> {
    // 工作库根目录（data/）用于放 db；init.json 另有自己的目录（桌面=exe 同目录）
    let dr = data_root(app)?;
    let _ = fs::create_dir_all(&dr);
    let init_path = init_json_path(app)?;
    if let Some(p) = init_path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let mut cfg = read_init(&init_path);

    // 工作库文件不存在（全新安装被清理/误删）→ 无论 init 是否标记完成，都强制迁移，
    // 否则 base 基础词会整片丢失。
    let working_path = crate::english::phrase::get_db_path(app)?;
    if !working_path.exists() {
        cfg.migration_done = false;
    }

    if cfg.base_db_version != BASE_DB_VERSION {
        cfg.migration_done = false;
    }

    if !cfg.migration_done {
        match run_migration(app) {
            Ok(()) => {
                cfg.base_db_version = BASE_DB_VERSION;
                cfg.migration_done = true;
                write_init(&init_path, &cfg)?;
            }
            Err(e) => {
                eprintln!(
                    "[init] 基础词迁移失败（种子可能缺失），跳过本次迁移，下次启动重试：{}",
                    e
                );
                // 不写 init（保持 migration_done=false 以便重试），不阻断启动
                return Ok(());
            }
        }
    } else {
        // 已迁移：确保 init.json 落盘（避免首次创建后崩溃未写入）
        let _ = write_init(&init_path, &cfg);
    }
    Ok(())
}

/// 暴露为 Tauri 命令：由前端在启动时（已显示窗口 + 全局遮罩）调用。
/// 与 ensure_init_and_migrate 分离，是为了把“初始化”从 .setup 阻塞中挪出来，
/// 让窗口先渲染、前端先弹出遮罩，再异步跑迁移，避免启动期白屏。
#[tauri::command]
pub fn ensure_init(app: tauri::AppHandle) -> Result<(), String> {
    ensure_init_and_migrate(&app)
}
