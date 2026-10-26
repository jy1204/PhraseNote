// ============================================================
// phrase.rs —— 短语数据库（PhrasePage.vue 相关后端命令）
// 这里集中存放所有操作 phrase_table 的逻辑：
//   - get_db_path：短语 db 的物理路径（桌面/移动端）
//   - save_phrase / load_all_phrases / delete_phrase：短语的增删查
//   - resolve_duplicate_overwrite：导入冲突时覆盖短语列表
//   - resolve_duplicates_batch：批量冲突裁决（一次 invoke 处理全部）
// 这些命令由前端 PhrasePage.vue 直接 invoke（或导入流程间接调用）。
// ============================================================

use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use serde_json;

/// 🖥️桌面端Windows：exe所在目录 → ./data/subject/english/phrase_db.db
#[cfg(desktop)]
pub fn get_db_path(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;

    let dir = exe_dir.join("data").join("subject").join("english");
    // 获取的路径文件phrase_db.db
    let db_file = dir.join("phrase_db.db");
    println!("[Desktop] db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

/// 📱手机端Android：app_data_dir/subject/english/phrase_db.db
#[cfg(mobile)]
pub fn get_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = data_dir.join("subject").join("english");
    let db_file = dir.join("phrase_db.db");
    println!("[Mobile] db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

// 保存一条 phrase 记录
#[tauri::command(rename_all = "snake_case")]
pub fn save_phrase(
    app: tauri::AppHandle,
    r#type: String,
    first_char: String,
    keyboard: String,
    list: String,
) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS phrase_table (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT,
            first_char TEXT,
            keyboard TEXT,
            list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM phrase_table WHERE keyboard = ?1)",
            params![keyboard],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        return Err(format!("关键词「{}」已存在", keyboard));
    }

    conn.execute(
        "INSERT INTO phrase_table (type, first_char, keyboard, list) VALUES (?1, ?2, ?3, ?4)",
        params![r#type, first_char, keyboard, list],
    )
    .map_err(|e| e.to_string())?;

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
            keyboard TEXT,
            list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, type, first_char, keyboard, list FROM phrase_table ORDER BY keyboard ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_,i64>(0)?,
                "type": row.get::<_,String>(1)?,
                "first_char": row.get::<_,String>(2)?,
                "keyboard": row.get::<_,String>(3)?,
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

// 删除短语
#[tauri::command(rename_all = "snake_case")]
pub fn delete_phrase(app: tauri::AppHandle, keyboard: String) -> Result<(), String> {
    let db_path = get_db_path(&app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    let affected = conn
        .execute(
            "DELETE FROM phrase_table WHERE keyboard = ?1",
            params![keyboard],
        )
        .map_err(|e| e.to_string())?;

    if affected == 0 {
        return Err(format!("未找到关键词「{}」", keyboard));
    }

    Ok(())
}

// 导入冲突时：用新列表覆盖已有短语
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
        conn.execute(
            "UPDATE phrase_table SET list = ?1 WHERE keyboard = ?2",
            params![new_list, keyword],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

// 追加合并核心逻辑：按 phrase 去重合并 new_list 进已有 list（供单条命令与批量命令共用）
fn apply_append(conn: &Connection, keyword: &str, list: &str) -> Result<(), String> {
    // 取出已有 list（关键词不存在时视为空数组）
    let existing: String = conn
        .query_row(
            "SELECT list FROM phrase_table WHERE keyboard = ?1",
            params![keyword],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());

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
    conn.execute(
        "UPDATE phrase_table SET list = ?1 WHERE keyboard = ?2",
        params![merged, keyword],
    )
    .map_err(|e| e.to_string())?;
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
        let total = items.len();
        let mut resolved = 0usize;
        let mut errors: Vec<serde_json::Value> = Vec::new();

        for item in &items {
            let r = if action == "overwrite" {
                conn.execute(
                    "UPDATE phrase_table SET list = ?1 WHERE keyboard = ?2",
                    params![item.new_list, item.keyword],
                )
                .map(|_| ())
                .map_err(|e| e.to_string())
            } else {
                apply_append(&conn, &item.keyword, &item.new_list)
            };
            match r {
                Ok(()) => resolved += 1,
                Err(e) => errors.push(serde_json::json!({
                    "keyword": item.keyword,
                    "error": e,
                })),
            }
        }

        Ok(serde_json::json!({
            "total": total,
            "resolved": resolved,
            "errors": errors,
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
            keyboard TEXT,
            list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    // 按首字母 / 中文构造 WHERE 条件；占位符编号与参数数组严格对齐：
    //   * / # 分支：无过滤参数，LIMIT/OFFSET 用 ?1/?2，参数 [limit, offset]
    //   字母分支：过滤用 ?1，LIMIT/OFFSET 用 ?2/?3，参数 [char, limit, offset]
    //   SELECT 与 COUNT 使用同一套编号，避免 rusqlite “Wrong number of parameters” 报错
    let char_filter = char.trim();
    let is_letter = !(char_filter.is_empty() || char_filter == "*" || char_filter == "#");

    let where_sql: String = if is_letter {
        String::from("WHERE lower(substr(keyboard,1,1)) = lower(?1)")
    } else if char_filter == "#" {
        // 中文首字符：CJK 统一表意文字区间（首字符直接比较，避免依赖 unicode() 函数）
        String::from("WHERE substr(keyboard,1,1) >= '一' AND substr(keyboard,1,1) <= '鿿'")
    } else {
        String::new()
    };

    let select_sql = if is_letter {
        format!(
            "SELECT id, type, first_char, keyboard, list FROM phrase_table {} ORDER BY keyboard ASC LIMIT ?2 OFFSET ?3",
            where_sql
        )
    } else {
        format!(
            "SELECT id, type, first_char, keyboard, list FROM phrase_table {} ORDER BY keyboard ASC LIMIT ?1 OFFSET ?2",
            where_sql
        )
    };
    let mut stmt = conn.prepare(&select_sql).map_err(|e| e.to_string())?;

    let map_row = |row: &rusqlite::Row| -> rusqlite::Result<serde_json::Value> {
        Ok(serde_json::json!({
            "id": row.get::<_,i64>(0)?,
            "type": row.get::<_,String>(1)?,
            "first_char": row.get::<_,String>(2)?,
            "keyboard": row.get::<_,String>(3)?,
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

    let count_sql = format!("SELECT COUNT(*) FROM phrase_table {}", where_sql);
    let total: i64 = if is_letter {
        conn.query_row(&count_sql, params![char_filter], |row| row.get(0))
    } else {
        conn.query_row(&count_sql, [], |row| row.get(0))
    }
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "rows": rows_vec, "total": total }))
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
            keyboard TEXT,
            list TEXT
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    // 模糊匹配：%keyword%；SQLite 的 LIKE 对 ASCII 默认不区分大小写
    let pattern = format!("%{}%", keyword);

    let mut stmt = conn
        .prepare(
            "SELECT id, type, first_char, keyboard, list FROM phrase_table WHERE keyboard LIKE ?1 ORDER BY keyboard ASC LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![pattern, limit, offset], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_,i64>(0)?,
                "type": row.get::<_,String>(1)?,
                "first_char": row.get::<_,String>(2)?,
                "keyboard": row.get::<_,String>(3)?,
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
            "SELECT COUNT(*) FROM phrase_table WHERE keyboard LIKE ?1",
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
            "SELECT EXISTS(SELECT 1 FROM phrase_table WHERE keyboard = ?1)",
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
                keyboard TEXT,
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
