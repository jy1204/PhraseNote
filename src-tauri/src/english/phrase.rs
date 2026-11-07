// ============================================================
// phrase.rs —— 短语数据库（PhrasePage.vue 相关后端命令）
// 这里集中存放所有操作 phrase_table 的逻辑：
//   - get_db_path：短语 db 的物理路径（桌面/移动端）
//   - save_phrase / load_all_phrases / delete_phrase：短语的增删查
//   - resolve_duplicate_overwrite：导入冲突时覆盖短语列表
//   - resolve_duplicates_batch：批量冲突裁决（一次 invoke 处理全部）
//   - batch_entry_phrases：批处理录入（解析 AI 生成的 JSON 文本，按双行两态模型自动合并）
// 这些命令由前端 PhrasePage.vue 直接 invoke（或导入流程间接调用）。
// ============================================================

use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid;
use serde_json;

/// 返回“数据根目录”（工作库与 init.json 的共同父目录）。
/// 桌面 = exe 所在目录下的 ./data；移动端 = app_data_dir（无需再加 data 层级）。
/// 供 get_db_path 与 init 模块复用，保证路径一致。
#[cfg(desktop)]
pub fn get_data_root(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;
    Ok(exe_dir.join("data"))
}

#[cfg(mobile)]
pub fn get_data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?)
}

/// 🖥️桌面端Windows：exe所在目录 → ./data/subject/english/phrase_db.db
#[cfg(desktop)]
pub fn get_db_path(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = get_data_root(_app)?.join("subject").join("english");
    // 获取的路径文件phrase_db.db
    let db_file = dir.join("phrase_db.db");
    println!("[Desktop] db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

/// 📱手机端Android：app_data_dir/subject/english/phrase_db.db
#[cfg(mobile)]
pub fn get_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = get_data_root(app)?.join("subject").join("english");
    let db_file = dir.join("phrase_db.db");
    println!("[Mobile] db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

/// 可见短语集合（列表 / 搜索 / 居中窗口共用）。
/// state=1 表示 base 被 user 覆盖 / 被用户删除（屏蔽态）—— 不参与显示与判重：
/// 列表/搜索/窗口查询、导入判重、手动保存/删除/迁移的探测统一用
/// `(source='user' OR (source='base' AND state=0))` 过滤。
/// ⚠【终态铁律】只作用于「启动迁移 / 删除 / 手动保存」三条路径：这三条不再改动
///    屏蔽行（不刷内容、不重置回 0、不删除、不复活）。
///    导入 / 接收 / 批处理的重复关键词处理按 `a23ffea` 口径，屏蔽行可作为
///    合并或元数据来源（详见《软件具体流程.md》1.2 例外小节）。
/// ⚠ 不再做 NOT EXISTS 相关子查询：state 本身就是屏蔽标记，被覆盖的 base 在
///    save_phrase / apply_append / delete_phrase / 导入时已同步标 state=1，
///    逐行再去 user 表查一遍是冗余的 O(N²)。历史库若漏标，由
///    ensure_phrase_indexes() 的自愈补上，避免同 keyword 出现 base + user 双行。
pub(crate) const VISIBLE_UNION_SQL: &str = "SELECT id, type, first_char, keyword, list FROM phrase_table WHERE source='user' UNION ALL SELECT id, type, first_char, keyword, list FROM phrase_table WHERE source='base' AND state=0";

/// 确保工作库上有短语查询所需索引（幂等，可重复调用）。
/// ⚠ 索引必须建在【工作库】上：种子库是只读打开的，且 run_migration 是逐行
///    INSERT/UPDATE 把种子数据搬进工作库，schema 对象不会随数据复制过去。
///    所以给 gen_seed.py / 种子 db 加索引对工作库毫无作用。
/// 2890 行实测：无索引 ≈710ms/次；建 (source, keyword) 后降到毫秒级（见《软件具体流程.md》1.2）。
/// 忽略失败（并发建索引可能抢 schema 锁），下次调用会自动重试，不阻断加载。
pub fn ensure_phrase_indexes(conn: &Connection) {
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_phrase_source_keyword ON phrase_table(source, keyword)",
        [],
    );
    // 兜底自愈：历史库 / 外部导入若存在「已有 user 覆盖行、但 base 仍是 state=0」的漏标，
    // 补标屏蔽。读取侧已改为只按 state 判定，漏标会让同一 keyword 显示两行。
    // 先探测（走覆盖索引，正常库零写入），命中才 UPDATE；表不存在时直接跳过。
    let need_fix: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM phrase_table b WHERE b.source='base' AND b.state=0 \
             AND EXISTS(SELECT 1 FROM phrase_table u WHERE u.source='user' AND u.keyword=b.keyword))",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    if need_fix {
        let _ = conn.execute(
            "UPDATE phrase_table SET state=1 WHERE rowid IN (\
               SELECT b.rowid FROM phrase_table b JOIN phrase_table u \
                 ON u.source='user' AND u.keyword=b.keyword \
               WHERE b.source='base' AND b.state=0)",
            [],
        );
    }
}

// 导出：只抽取 source='user' 的行，生成一份临时 db 的字节流。
// base 种子（source='base'）不随个人备份 / 互传走，符合双行两态模型约定。
// 返回字节可直接作为 zip 条目写入，避免把 base 行与 state=1 隐藏标记带进导出包。
pub fn export_user_rows_bytes(db_path: &Path) -> Result<Vec<u8>, String> {
    // rusqlite 无直接内存序列化 API，落到临时文件再读回字节。
    // 临时文件放在源库同目录（app 自身数据目录，各平台均可写，含 Android），避免 temp_dir 不可写。
    let tmp_path = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!("phrase_export_{}.db", uuid::Uuid::new_v4()));
    let _ = fs::remove_file(&tmp_path);

    // 打开【源库】（含真实数据），挂接临时库，把 source='user' 的行抽到临时库。
    // 注意：务必从源库 SELECT，而非从刚创建的空临时库 SELECT（否则抽出 0 行）。
    let src = Connection::open(db_path).map_err(|e| e.to_string())?;
    src.execute(
        "ATTACH DATABASE ? AS export_tmp",
        params![tmp_path.to_string_lossy().to_string()],
    )
    .map_err(|e| e.to_string())?;
    src.execute(
        "CREATE TABLE IF NOT EXISTS export_tmp.phrase_table (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT, first_char TEXT, keyword TEXT, list TEXT,
            source TEXT DEFAULT 'user', state INTEGER DEFAULT 0
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    src.execute(
        "INSERT INTO export_tmp.phrase_table (type, first_char, keyword, list, source, state)
         SELECT type, first_char, keyword, list, source, state
         FROM phrase_table WHERE source='user'",
        [],
    )
    .map_err(|e| e.to_string())?;
    src.execute("DETACH DATABASE export_tmp", [])
        .map_err(|e| e.to_string())?;

    let bytes = fs::read(&tmp_path).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&tmp_path);
    Ok(bytes)
}

// 保存一条 phrase 记录
#[tauri::command(rename_all = "snake_case")]
pub fn save_phrase(
    app: tauri::AppHandle,
    r#type: String,
    first_char: String,
    keyword: String,
    list: String,
) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
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

    // 双行模型路由：base 只读，用户改动一律落 user 行
    //   - 已有 user 行 → 原地更新
    //   - 已有 base 行 → 另建 user 覆盖行，并把对应 base 标 state=1（内容不动）
    //   - 全新关键词 → 直接插入 user 行
    let existing_source: Option<String> = conn
        .query_row(
            // 跳过被屏蔽的 base 行：被删过的 base 词重新添加应算「全新关键词」。
            // ORDER BY 让 user 行优先，消除双行并存时 LIMIT 1 命中不确定的问题。
            "SELECT source FROM phrase_table WHERE keyword = ?1 AND (source='user' OR (source='base' AND state=0)) ORDER BY (source='user') DESC LIMIT 1",
            params![keyword],
            |row| row.get(0),
        )
        .ok();

    match existing_source {
        Some(s) if s == "user" => {
            conn.execute(
                "UPDATE phrase_table SET type=?1, first_char=?2, list=?3 WHERE source='user' AND keyword=?4",
                params![r#type, first_char, list, keyword],
            )
            .map_err(|e| e.to_string())?;
        }
        Some(_) => {
            // 覆盖 base：建 user 行 + 标记 base 隐藏
            conn.execute(
                "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) VALUES (?1,?2,?3,?4,'user',0)",
                params![r#type, first_char, keyword, list],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword=?1 AND state=0",
                params![keyword],
            )
            .map_err(|e| e.to_string())?;
        }
        None => {
            conn.execute(
                "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) VALUES (?1,?2,?3,?4,'user',0)",
                params![r#type, first_char, keyword, list],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

// 加载所有短语
#[tauri::command(rename_all = "snake_case")]
pub fn load_all_phrases(app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
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

    let mut stmt = conn
        .prepare(
            // 跳过被屏蔽的 base 行（state=1），与列表/搜索/窗口口径一致
            "SELECT id, type, first_char, keyword, list FROM phrase_table WHERE source='user' OR (source='base' AND state=0) ORDER BY keyword ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_,i64>(0)?,
                "type": row.get::<_,String>(1)?,
                "first_char": row.get::<_,String>(2)?,
                "keyword": row.get::<_,String>(3)?,
                "list": row.get::<_,String>(4)?
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for r in rows {
        result.push(r.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

// 删除短语（双行模型路由）
#[tauri::command(rename_all = "snake_case")]
pub fn delete_phrase(app: tauri::AppHandle, keyword: String) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    //   - 先删 user 行（覆盖类或纯 user 行）；删掉后把对应 base 标 state=1 隐藏（不改 base 内容）
    //   - 若没有 user 行 → 这是 base 词，软隐藏（state=1）而非真删
    let user_deleted = conn
        .execute(
            "DELETE FROM phrase_table WHERE source='user' AND keyword = ?1",
            params![keyword],
        )
        .map_err(|e| e.to_string())?;

    if user_deleted > 0 {
        conn.execute(
            "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword=?1 AND state=0",
            params![keyword],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        let base_updated = conn
            .execute(
                "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword = ?1 AND state=0",
                params![keyword],
            )
            .map_err(|e| e.to_string())?;
        if base_updated == 0 {
            // 没有可见行可删：若只剩已屏蔽的 base（终态），按幂等成功处理，绝不回写 / 删除它
            let hidden: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM phrase_table WHERE source='base' AND keyword=?1 AND state=1)",
                    params![keyword],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if !hidden {
                return Err(format!("未找到关键词「{}」", keyword));
            }
        }
        Ok(())
    }
}

// 导入冲突「覆盖」：list 只放导入内容，不含 base 原有短语。
//   · 已有 user 行 → 整段替换 list（type/first_char 保留库内值）
//   · 无 user 行（只有 base）→ 从同名 base 行只读复制 type/first_char 建 user 行
// 两种情况都先幂等屏蔽同名「可见」base：防止历史漏标时 base + user 双行同时显示。
// 屏蔽态 base(state=1) 不参与判重，也不会被这条 UPDATE 改动。
fn apply_overwrite(conn: &Connection, keyword: &str, new_list: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword=?1 AND state=0",
        params![keyword],
    )
    .map_err(|e| e.to_string())?;

    let updated = conn
        .execute(
            "UPDATE phrase_table SET list = ?1 WHERE source='user' AND keyword = ?2",
            params![new_list, keyword],
        )
        .map_err(|e| e.to_string())?;
    if updated > 0 {
        return Ok(());
    }

    // 无 user 行：从同名 base 复制 type/first_char（只读，不改动那一行），
    // list 直接放导入内容 —— 这正是「覆盖」与「追加」的差别。
    let inserted = conn
        .execute(
            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
             SELECT type, first_char, ?1, ?2, 'user', 0 FROM phrase_table WHERE keyword=?1 AND source='base' LIMIT 1",
            params![keyword, new_list],
        )
        .map_err(|e| e.to_string())?;
    if inserted == 0 {
        // 连 base 都没有（裁决期间数据被删）：兜底建行，避免 0 行插入静默丢数据
        let first_char = keyword
            .chars()
            .next()
            .map(|c| c.to_lowercase().collect::<String>())
            .unwrap_or_default();
        conn.execute(
            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
             VALUES ('A~Z', ?1, ?2, ?3, 'user', 0)",
            params![first_char, keyword, new_list],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// 导入冲突时：用新列表覆盖已有短语（单条版，与批量裁决共用 apply_overwrite）
// async + spawn_blocking：SQLite IO 挪到阻塞线程池，不卡主线程（同步命令会冻住 UI，点击后转圈刷不出来）
#[tauri::command(rename_all = "snake_case")]
pub async fn resolve_duplicate_overwrite(
    app: tauri::AppHandle,
    keyword: String,
    new_list: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let db_path = get_db_path(&app)?;
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        apply_overwrite(&conn, &keyword, &new_list)
    })
    .await
    .map_err(|e| e.to_string())?
}

// 追加合并核心逻辑：按 phrase 去重合并 new_list 进已有 list（供单条命令与批量命令共用）
// base 只读：若目标行是 base，则把合并结果落为 user 覆盖行，并把 base 标 state=1
fn apply_append(conn: &Connection, keyword: &str, list: &str) -> Result<(), String> {
    // 取现有 list 与是否为 base 行（优先 user 行；无则 base 行）
    let row: Option<(String, bool)> = conn
        .query_row(
            // 跳过被屏蔽的 base：否则会把用户已删除/已覆盖的旧 list 又合并回来
            "SELECT list, (source='base') FROM phrase_table WHERE keyword=?1 AND (source='user' OR (source='base' AND state=0)) ORDER BY source DESC LIMIT 1",
            params![keyword],
            |r| Ok((r.get::<_,String>(0)?, r.get::<_,bool>(1)?)),
        )
        .ok();

    let (existing, is_base) = match row {
        Some((l, b)) => (l, b),
        None => ("[]".to_string(), false),
    };

    let mut existing_vec: Vec<serde_json::Value> =
        serde_json::from_str(&existing).unwrap_or_default();
    let new_vec: Vec<serde_json::Value> =
        serde_json::from_str(list).map_err(|e| e.to_string())?;

    // 以 phrase 为 key 去重：先收集已有，再并入新内容里不重复的项
    let mut seen: HashSet<String> = existing_vec
        .iter()
        .filter_map(|v| v.get("phrase").and_then(|p| p.as_str()).map(|s| s.trim().to_string()))
        .collect();

    for item in new_vec {
        let phrase = item
            .get("phrase")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if phrase.is_empty() {
            continue;
        }
        if seen.contains(&phrase) {
            continue; // 内容中有重复的 key 就跳过
        }
        seen.insert(phrase);
        existing_vec.push(item);
    }

    let merged = serde_json::to_string(&existing_vec).map_err(|e| e.to_string())?;

    if is_base {
        // base 只读：落 user 覆盖行，base 标 state=1
        conn.execute(
            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
             SELECT type, first_char, ?1, ?2, 'user', 0 FROM phrase_table WHERE keyword=?1 AND source='base' LIMIT 1",
            params![keyword, merged],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword=?1 AND state=0",
            params![keyword],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let updated = conn
            .execute(
                "UPDATE phrase_table SET list = ?1 WHERE keyword = ?2 AND source='user'",
                params![&merged, keyword],
            )
            .map_err(|e| e.to_string())?;
        if updated == 0 {
            // 兜底：探测没命中、库里也没有 user 行（如裁决期间 base 被屏蔽），
            // 不能让导入内容静默丢失 —— 直接建一条 user 行。
            let first_char = keyword
                .chars()
                .next()
                .map(|c| c.to_lowercase().collect::<String>())
                .unwrap_or_default();
            conn.execute(
                "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
                 VALUES ('A~Z', ?1, ?2, ?3, 'user', 0)",
                params![first_char, keyword, &merged],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// 批量冲突裁决：一次 invoke 处理全部重复项（共用一个连接，避免 N 次 IPC 往返 + N 次 Connection::open）
#[derive(Debug, serde::Deserialize)]
pub struct DupResolveItem {
    pub keyword: String,
    pub new_list: String,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn resolve_duplicates_batch(
    app: tauri::AppHandle,
    action: String,
    items: Vec<DupResolveItem>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<serde_json::Value, String> {
        let db_path = get_db_path(&app)?;
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        // 整批共用一个事务：N 条写入只 fsync 一次，避免每条独立 autocommit 在 Windows 上逐个刷盘导致「转圈好几秒」
        conn.execute("BEGIN", []).map_err(|e| e.to_string())?;
        let total = items.len();
        let mut resolved = 0usize;
        let mut errors: Vec<serde_json::Value> = Vec::new();

        for item in &items {
            // 内层闭包捕获单条错误，保证一条失败只记 errors、不中断整批（仍统一 COMMIT）
            let r = (|| -> Result<(), String> {
                if action == "overwrite" {
                    // 与单条命令共用同一套覆盖语义（幂等屏蔽可见 base → 替换/新建 user 行）
                    apply_overwrite(&conn, &item.keyword, &item.new_list)?;
                } else {
                    apply_append(&conn, &item.keyword, &item.new_list)?;
                }
                Ok(())
            })();
            match r {
                Ok(()) => resolved += 1,
                Err(e) => errors.push(serde_json::json!({
                    "keyword": item.keyword,
                    "error": e,
                })),
            }
        }

        conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "total": total,
            "resolved": resolved,
            "errors": errors,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// ============================================================
// 批处理录入（帮助 → 批处理短语）：把 AI 生成的 JSON 文本按双行两态
// 模型自动合并入库（无冲突弹窗，固定规则，适合连续大批量录入）。
//   · 解析容错：剥 markdown 围栏 → 直接 parse → 失败截取首尾花括号再 parse
//   · 归一化：type 只认 "A~Z"/"#"（否则按 keyword 首字符推断）；
//              first_char 不轻信输入，一律从 keyword 重新推导；
//              translate 缺省/非字符串 → ""；"= xxx" 同义项原样保留
//   · 去重：同批内与库内均按 phrase 大小写不敏感去重
//   · 合并规则（与 save_phrase / apply_append 同源）：
//       库内无行      → 直接插 user 行（新增）
//       已有 user 行  → 去重追加进该行（追加）
//       仅 base 行    → 插 user 覆盖行（base.list + 新内容），base 标 state=1（合并）
//   · 同批内相同 keyword 先合并；单命令单事务单连接，避免 N 次开库与 SQLITE_BUSY
// ============================================================

// 单条录入失败信息（keyword + 原因），供前端在浮窗里展示
#[derive(Debug, serde::Serialize)]
pub struct BatchEntryFailure {
    pub keyword: String,
    pub reason: String,
}

// 归一化后的单组短语（keyword / type / first_char / list[{phrase, translate}]）
#[derive(Debug)]
struct BatchEntry {
    keyword: String,
    entry_type: String,
    first_char: String,
    list: Vec<serde_json::Value>,
}

// 容错解析：从文本中取出 phrases 数组（Value 形式），失败返回可读原因
fn parse_batch_json(text: &str) -> Result<Vec<serde_json::Value>, String> {
    let mut s = text.trim();
    if s.is_empty() {
        return Err("请先粘贴内容".to_string());
    }
    // 剥掉 markdown 代码围栏（```json ... ``` 或裸 ```）：去掉首行与结尾的 ```
    if s.starts_with("```") {
        if let Some(nl) = s.find('\n') {
            s = s[nl + 1..].trim_start();
        }
        if s.ends_with("```") {
            s = s[..s.len() - 3].trim_end();
        }
    }
    // 直接 parse；失败则截取第一个 { 到最后一个 } 再 parse（兜住 AI 在 JSON 前后加的说明文字）
    let value = match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => v,
        Err(e1) => {
            let start = s.find('{');
            let end = s.rfind('}');
            match (start, end) {
                (Some(a), Some(b)) if b > a => serde_json::from_str::<serde_json::Value>(&s[a..=b])
                    .map_err(|e2| format!("JSON 解析失败：{}", e2))?,
                _ => return Err(format!("JSON 解析失败：{}", e1)),
            }
        }
    };
    value
        .get("phrases")
        .and_then(|p| p.as_array())
        .cloned()
        .ok_or_else(|| "JSON 中缺少 phrases 数组".to_string())
}

// 归一化单组：type 推断、first_char 重新推导、list 清洗与组内去重
fn normalize_batch_entry(raw: &serde_json::Value) -> Result<BatchEntry, String> {
    let keyword = raw
        .get("keyword")
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if keyword.is_empty() {
        return Err("缺少关键词".to_string());
    }

    // type：只认 "A~Z" / "#"，缺失或非法时按 keyword 首字符推断
    let entry_type = match raw.get("type").and_then(|t| t.as_str()).unwrap_or("").trim() {
        "A~Z" => "A~Z".to_string(),
        "#" => "#".to_string(),
        _ => {
            if keyword
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            {
                "A~Z".to_string()
            } else {
                "#".to_string()
            }
        }
    };

    // first_char：一律从 keyword 重新推导（AI 可能漏填/填错）
    //   A~Z → keyword 中第一个英文字母，转小写；# → keyword 第一个字符
    let first_char = if entry_type == "A~Z" {
        keyword
            .chars()
            .find(|c| c.is_ascii_alphabetic())
            .map(|c| c.to_ascii_lowercase().to_string())
            .unwrap_or_default()
    } else {
        keyword
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_default()
    };
    if first_char.is_empty() {
        return Err("无法推导首字符".to_string());
    }

    // list：phrase trim 后必填；translate 缺省/非字符串 → ""；组内按 phrase 大小写不敏感去重
    let mut list: Vec<serde_json::Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(arr) = raw.get("list").and_then(|l| l.as_array()) {
        for item in arr {
            let phrase = item
                .get("phrase")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if phrase.is_empty() {
                continue;
            }
            if !seen.insert(phrase.to_lowercase()) {
                continue;
            }
            let translate = item
                .get("translate")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            list.push(serde_json::json!({ "phrase": phrase, "translate": translate }));
        }
    }
    if list.is_empty() {
        return Err("没有有效的短语".to_string());
    }

    Ok(BatchEntry {
        keyword,
        entry_type,
        first_char,
        list,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn batch_entry_phrases(
    app: tauri::AppHandle,
    json_text: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<serde_json::Value, String> {
        // ① 解析 + 归一化（纯内存，不写库）；单条无效只记失败，不影响其他条目
        let raw_items = parse_batch_json(&json_text)?;
        if raw_items.is_empty() {
            return Err("没有可录入的内容".to_string());
        }
        let mut failures: Vec<BatchEntryFailure> = Vec::new();
        let mut entries: Vec<BatchEntry> = Vec::new();
        for raw in &raw_items {
            match normalize_batch_entry(raw) {
                Ok(e) => entries.push(e),
                Err(reason) => failures.push(BatchEntryFailure {
                    keyword: raw
                        .get("keyword")
                        .and_then(|k| k.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    reason,
                }),
            }
        }

        // ② 同批内相同 keyword 先合并（保持首次出现顺序；type/first_char 由 keyword 决定，天然一致）
        let mut order: Vec<String> = Vec::new();
        let mut by_keyword: std::collections::HashMap<String, BatchEntry> =
            std::collections::HashMap::new();
        for e in entries {
            if let Some(dst) = by_keyword.get_mut(&e.keyword) {
                let mut seen: HashSet<String> = dst
                    .list
                    .iter()
                    .filter_map(|v| v.get("phrase").and_then(|p| p.as_str()).map(|s| s.to_lowercase()))
                    .collect();
                for item in e.list {
                    let key = item
                        .get("phrase")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if seen.insert(key) {
                        dst.list.push(item);
                    }
                }
            } else {
                order.push(e.keyword.clone());
                by_keyword.insert(e.keyword.clone(), e);
            }
        }

        // ③ 开库 + 单事务写入（N 组只开一次库、只 fsync 一次）
        let db_path = get_db_path(&app)?;
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS phrase_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT, first_char TEXT, keyword TEXT, list TEXT,
                source TEXT DEFAULT 'user', state INTEGER DEFAULT 0
            )",
            [],
        )
        .map_err(|e| e.to_string())?;
        // 逐组按 keyword 查重，建索引加速（旧库无此索引时补建）
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_keyword ON phrase_table(keyword)",
            [],
        )
        .ok();

        conn.execute("BEGIN", []).map_err(|e| e.to_string())?;

        let mut added = 0usize; // 全新 keyword 组数
        let mut appended = 0usize; // 追加进已有 user 行的组数
        let mut merged_cnt = 0usize; // 与 base 合并（base 被隐藏）的组数
        let mut skipped_phrases = 0usize; // 库内已存在被跳过的短语条数
        let mut skipped_groups = 0usize; // 整组内容已全部存在的组数

        for keyword in &order {
            let entry = match by_keyword.get(keyword) {
                Some(e) => e,
                None => continue,
            };

            // 查库内现状：优先 user 行（'user' > 'base'，DESC 取前）；无则 base 行
            let existing: Option<(String, bool)> = conn
                .query_row(
                    "SELECT list, (source='base') FROM phrase_table WHERE keyword=?1 ORDER BY source DESC LIMIT 1",
                    params![keyword],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)),
                )
                .ok();

            // 库内该 keyword 已有短语集合（小写键），用于过滤与计数
            let existing_phrases: HashSet<String> = existing
                .as_ref()
                .map(|(l, _)| {
                    serde_json::from_str::<serde_json::Value>(l)
                        .ok()
                        .and_then(|v| v.as_array().cloned())
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|v| v.get("phrase").and_then(|p| p.as_str()).map(|s| s.to_lowercase()))
                        .collect()
                })
                .unwrap_or_default();

            // 过滤掉库内已存在的（大小写不敏感）
            let to_add: Vec<serde_json::Value> = entry
                .list
                .iter()
                .filter(|item| {
                    let p = item.get("phrase").and_then(|p| p.as_str()).unwrap_or("").to_lowercase();
                    !existing_phrases.contains(&p)
                })
                .cloned()
                .collect();
            skipped_phrases += entry.list.len() - to_add.len();
            if to_add.is_empty() {
                skipped_groups += 1;
                continue;
            }

            let list_json = serde_json::to_string(&to_add).map_err(|e| e.to_string())?;

            // 内层闭包隔离单组错误：一条写失败只记 failures，不中断整批（仍统一 COMMIT）
            let r = (|| -> Result<(), String> {
                match &existing {
                    None => {
                        // 全新技术 → 直接插 user 行
                        conn.execute(
                            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
                             VALUES (?1,?2,?3,?4,'user',0)",
                            params![entry.entry_type, entry.first_char, entry.keyword, list_json],
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    Some((_, false)) => {
                        // 已有 user 行 → 去重追加（复用 apply_append 的 UPDATE 分支）
                        apply_append(&conn, &entry.keyword, &list_json)?;
                    }
                    Some((existing_list, true)) => {
                        // 仅 base 行 → 插 user 覆盖行（base.list + 新内容），base 标 state=1。
                        // 不用 apply_append：它从 base 行复制 type/first_char，
                        // 这里用本批归一化后的值，保证与录入内容一致。
                        let mut base_vec: Vec<serde_json::Value> =
                            serde_json::from_str(existing_list).unwrap_or_default();
                        base_vec.extend(to_add.iter().cloned());
                        let merged_json = serde_json::to_string(&base_vec).map_err(|e| e.to_string())?;
                        conn.execute(
                            "INSERT INTO phrase_table (type, first_char, keyword, list, source, state) \
                             VALUES (?1,?2,?3,?4,'user',0)",
                            params![entry.entry_type, entry.first_char, entry.keyword, merged_json],
                        )
                        .map_err(|e| e.to_string())?;
                        conn.execute(
                            "UPDATE phrase_table SET state=1 WHERE source='base' AND keyword=?1",
                            params![entry.keyword],
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            })();

            match r {
                Ok(()) => match &existing {
                    None => added += 1,
                    Some((_, false)) => appended += 1,
                    Some((_, true)) => merged_cnt += 1,
                },
                Err(e) => failures.push(BatchEntryFailure {
                    keyword: entry.keyword.clone(),
                    reason: e,
                }),
            }
        }

        conn.execute("COMMIT", []).map_err(|e| e.to_string())?;

        Ok(serde_json::json!({
            "total_groups": order.len(),
            "added": added,
            "appended": appended,
            "merged": merged_cnt,
            "skipped_phrases": skipped_phrases,
            "skipped_groups": skipped_groups,
            "failed": failures,
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

// 分页加载短语：可按 char 过滤（* 全部 / # 中文首字符 / 单个字母 A~Z）
// 返回当前页数据 rows 与总数 total（前端切换类型、滚动懒加载用）
#[tauri::command(rename_all = "snake_case")]
pub fn load_phrases_page(
    app: tauri::AppHandle,
    char: String,
    offset: i64,
    limit: i64,
) -> Result<serde_json::Value, String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
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
    // 老库升级后首次打开短语页自动补索引（纯代码自愈，无需替换 db 文件）
    ensure_phrase_indexes(&conn);

    // 按首字母 / 中文构造 WHERE 条件；占位符编号与参数数组严格对齐：
    //   * / # 分支：无过滤参数，LIMIT/OFFSET 用 ?1/?2，参数 [limit, offset]
    //   字母分支：过滤用 ?1，LIMIT/OFFSET 用 ?2/?3，参数 [char, limit, offset]
    //   SELECT 与 COUNT 使用同一套编号，避免 rusqlite “Wrong number of parameters” 报错
    let char_filter = char.trim();
    let is_letter = !(char_filter.is_empty() || char_filter == "*" || char_filter == "#");

    // 双行模型加载：user 行恒显示；base 行只有 state=0 才显示。
    // state=1（被覆盖 / 用户删除）即屏蔽态，直接跳过，不再做 NOT EXISTS 相关子查询。
    let union_sql = VISIBLE_UNION_SQL;

    let inner_filter: String = if is_letter {
        String::from("WHERE lower(substr(sub.keyword,1,1)) = lower(?1)")
    } else if char_filter == "#" {
        // 中文首字符：CJK 统一表意文字区间（首字符直接比较，避免依赖 unicode() 函数）
        String::from("WHERE substr(sub.keyword,1,1) >= '一' AND substr(sub.keyword,1,1) <= '鿿'")
    } else {
        String::new()
    };

    // 外层套过滤 + 排序 + 分页；列序保持 id,type,first_char,keyword,list 与前端一致
    let select_sql = format!(
        "SELECT * FROM ({}) sub {} ORDER BY sub.keyword ASC {}",
        union_sql,
        inner_filter,
        if is_letter {
            "LIMIT ?2 OFFSET ?3"
        } else {
            "LIMIT ?1 OFFSET ?2"
        }
    );
    let mut stmt = conn.prepare(&select_sql).map_err(|e| e.to_string())?;

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<serde_json::Value> {
        Ok(serde_json::json!({
            "id": row.get::<_,i64>(0)?,
            "type": row.get::<_,String>(1)?,
            "first_char": row.get::<_,String>(2)?,
            "keyword": row.get::<_,String>(3)?,
            "list": row.get::<_,String>(4)?
        }))
    };

    let rows = if is_letter {
        stmt.query_map(params![char_filter, limit, offset], map_row)
    } else {
        stmt.query_map(params![limit, offset], map_row)
    }
    .map_err(|e| e.to_string())?;

    let mut rows_vec = Vec::new();
    for r in rows {
        rows_vec.push(r.map_err(|e| e.to_string())?);
    }

    let count_sql = format!(
        "SELECT COUNT(*) FROM ({}) sub {}",
        union_sql, inner_filter
    );
    let total: i64 = if is_letter {
        conn.query_row(&count_sql, params![char_filter], |row| row.get(0))
    } else {
        conn.query_row(&count_sql, [], |row| row.get(0))
    }
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "rows": rows_vec, "total": total }))
}

// 以某个关键词为锚点，取「上下合计 limit 条」的居中窗口（编辑保存后刷新用）。
// 一次 IPC 同时返回窗口数据 / 总数 / 锚点排名 / 窗口绝对起点，前端不再需要
// 从 offset=0 重查后逐页补齐，也不必事后滚回修改位置。
// 筛选规则（* 全局 / # 中文首字符 / 单个字母）与 load_phrases_page 完全一致。
#[tauri::command(rename_all = "snake_case")]
pub fn load_phrase_window(
    app: tauri::AppHandle,
    keyword: String,
    char: String,
    limit: i64,
) -> Result<serde_json::Value, String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
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
    ensure_phrase_indexes(&conn);

    let char_filter = char.trim();
    let is_letter = !(char_filter.is_empty() || char_filter == "*" || char_filter == "#");
    let limit = if limit <= 0 { 100 } else { limit };

    // 被屏蔽(state=1)的 base 直接跳过，不再对它做 NOT EXISTS 相关子查询
    let union_sql = VISIBLE_UNION_SQL;

    let inner_filter: String = if is_letter {
        String::from("WHERE lower(substr(sub.keyword,1,1)) = lower(?1)")
    } else if char_filter == "#" {
        String::from("WHERE substr(sub.keyword,1,1) >= '一' AND substr(sub.keyword,1,1) <= '鿿'")
    } else {
        String::new()
    };

    // 锚点排名：当前筛选下排在 keyword 之前的条数。
    // 与 ORDER BY sub.keyword ASC 同用 SQLite 默认 BINARY collation，边界不漏行。
    // letter 分支 ?1=字母、?2=关键词；其余分支 ?1=关键词（# 的中文条件无参数）。
    let rank_filter = if is_letter {
        format!("{} AND sub.keyword < ?2", inner_filter)
    } else if inner_filter.is_empty() {
        String::from("WHERE sub.keyword < ?1")
    } else {
        format!("{} AND sub.keyword < ?1", inner_filter)
    };
    let rank_sql = format!("SELECT COUNT(*) FROM ({}) sub {}", union_sql, rank_filter);
    let rank: i64 = if is_letter {
        conn.query_row(&rank_sql, params![char_filter, keyword.as_str()], |row| row.get(0))
    } else {
        conn.query_row(&rank_sql, params![keyword.as_str()], |row| row.get(0))
    }
    .map_err(|e| e.to_string())?;

    let count_sql = format!("SELECT COUNT(*) FROM ({}) sub {}", union_sql, inner_filter);
    let total: i64 = if is_letter {
        conn.query_row(&count_sql, params![char_filter], |row| row.get(0))
    } else {
        conn.query_row(&count_sql, [], |row| row.get(0))
    }
    .map_err(|e| e.to_string())?;

    // 窗口以锚点为中心（上下各约半屏），靠近首尾时夹到合法范围
    let max_start = (total - limit).max(0);
    let start_offset = (rank - limit / 2).clamp(0, max_start);

    let select_sql = if is_letter {
        format!(
            "SELECT * FROM ({}) sub {} ORDER BY sub.keyword ASC LIMIT ?2 OFFSET ?3",
            union_sql, inner_filter
        )
    } else {
        format!(
            "SELECT * FROM ({}) sub {} ORDER BY sub.keyword ASC LIMIT ?1 OFFSET ?2",
            union_sql, inner_filter
        )
    };
    let mut stmt = conn.prepare(&select_sql).map_err(|e| e.to_string())?;
    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<serde_json::Value> {
        Ok(serde_json::json!({
            "id": row.get::<_,i64>(0)?,
            "type": row.get::<_,String>(1)?,
            "first_char": row.get::<_,String>(2)?,
            "keyword": row.get::<_,String>(3)?,
            "list": row.get::<_,String>(4)?
        }))
    };
    let rows = if is_letter {
        stmt.query_map(params![char_filter, limit, start_offset], map_row)
    } else {
        stmt.query_map(params![limit, start_offset], map_row)
    }
    .map_err(|e| e.to_string())?;

    let mut rows_vec = Vec::new();
    for r in rows {
        rows_vec.push(r.map_err(|e| e.to_string())?);
    }

    Ok(serde_json::json!({
        "rows": rows_vec,
        "total": total,
        "rank": rank,
        "startOffset": start_offset,
    }))
}

// 全局搜索短语：跨全部数据按关键词模糊匹配，分页返回 { rows, total }
// （前端搜索框输入时调用，避免一次性加载全部；滚动到底继续翻页）
#[tauri::command(rename_all = "snake_case")]
pub fn search_phrases(
    app: tauri::AppHandle,
    keyword: String,
    offset: i64,
    limit: i64,
) -> Result<serde_json::Value, String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    conn.execute(
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
    ensure_phrase_indexes(&conn);

    // 模糊匹配：%keyword%；SQLite 的 LIKE 对 ASCII 默认不区分大小写
    let pattern = format!("%{}%", keyword);

    // 双行模型：搜索同样 user 行恒出、base 仅 state=0 才出。
    // 被屏蔽(state=1)的 base 直接跳过，不再做 NOT EXISTS 相关子查询。
    let union_sql = VISIBLE_UNION_SQL;

    let select_sql = format!(
        "SELECT * FROM ({}) sub WHERE sub.keyword LIKE ?1 ORDER BY sub.keyword ASC LIMIT ?2 OFFSET ?3",
        union_sql
    );
    let mut stmt = conn.prepare(&select_sql).map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![pattern, limit, offset], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_,i64>(0)?,
                "type": row.get::<_,String>(1)?,
                "first_char": row.get::<_,String>(2)?,
                "keyword": row.get::<_,String>(3)?,
                "list": row.get::<_,String>(4)?
            }))
        })
        .map_err(|e| e.to_string())?;

    let mut rows_vec = Vec::new();
    for r in rows {
        rows_vec.push(r.map_err(|e| e.to_string())?);
    }

    let total: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM ({}) sub WHERE sub.keyword LIKE ?1",
                union_sql
            ),
            params![pattern],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "rows": rows_vec, "total": total }))
}

// 判断关键词是否已存在（前端添加/修改时的重复检测，避免一次性加载全部）
#[tauri::command(rename_all = "snake_case")]
pub fn keyword_exists(app: tauri::AppHandle, keyword: String) -> Result<bool, String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    let exists: bool = conn
        .query_row(
            // 被屏蔽的 base 不算「已存在」：用户删掉该词后重新添加应走新建，而非撞词追加
            "SELECT EXISTS(SELECT 1 FROM phrase_table WHERE keyword = ?1 AND (source='user' OR (source='base' AND state=0)))",
            params![keyword],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(exists)
}

// 追加内容到已有词条：复用已有 list，按 phrase 去重后合并写回
// async + spawn_blocking：JSON 合并 + SQLite IO 挪到阻塞线程池，不卡主线程
#[tauri::command(rename_all = "snake_case")]
pub async fn append_phrase(
    app: tauri::AppHandle,
    keyword: String,
    list: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let db_path = get_db_path(&app)?;
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS phrase_table (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT,
                first_char TEXT,
                keyword TEXT,
                list TEXT
            )",
            [],
        )
        .map_err(|e| e.to_string())?;
        apply_append(&conn, &keyword, &list)
    })
    .await
    .map_err(|e| e.to_string())?
}
