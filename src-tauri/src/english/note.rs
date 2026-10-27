// ============================================================
// note.rs —— 笔记数据库（NotePage.vue 相关后端命令）
// 仿照 phrase.rs 实现，集中存放操作 note_db 的逻辑。
//
// 数据库设计（按「目录 / 文件 / 内容」顺序，统一在一张文件表里存储）：
//   folder_table  —— 目录(id / name / deletable / renamable)
//   note_table    —— 文件 + 内容(id / folder_id / title / date / content)
//                    正文 content 直接作为列存于文件表，不另拆表；
//                    目录与文件一对多，删目录级联删其下文件。
//
// 路径：data/subject/english/note_db.db（桌面在 exe 旁，移动端在 app_data_dir）
//
// ⚠️ 重要约定（曾踩坑）：本文件所有命令带 rename_all="snake_case"，
//    前端 invoke 的「参数名」必须用 snake_case（folder_id / note_ids / note_id），
//    不能写 camelCase（folderId / noteIds / noteId），否则 Tauri 会因参数名不匹配
//    而拒绝执行，导致文件/内容写不进数据库（错误会被前端 .catch 吞掉、不易察觉）。
//
// 命令清单：
//   create_folder       —— 后端新建目录并生成 id（f+UUID），返回新 id
//   create_note         —— 后端新建文件并生成 id（n+UUID），同时落库 title（由前端传入），返回新 id
//   load_note_tree      —— 加载目录 + 文件「元数据」(id/title/date)，不含正文；
//                          正文只在点击文件时由 load_note_content 按需读取。
//   load_note_content   —— 点击/打开某篇文件时，按 note_id 读取其正文 content。
//   save_folder         —— 目录改名（upsert，仅更新 name/deletable/renamable）
//   save_note           —— 文件改名 / 移动（仅更新 title/date/folder_id，不碰 content）
//   delete_folder       —— 删除目录（级联删其下所有文件）
//   delete_note         —— 删除单个文件
//   move_note           —— 移动单个文件到其他目录
//   batch_move_notes    —— 批量移动（事务）
//   batch_delete_notes  —— 批量删除（事务）
//   update_note_content —— 单独保存某篇文件的正文（UPDATE note_table.content）
// ============================================================

use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde_json;
use tauri::Manager;
use uuid::Uuid;

/// 🖥️桌面端Windows：exe所在目录 → ./data/subject/english/note_db.db
#[cfg(desktop)]
pub fn get_db_path(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("无法获取exe所在目录")?;

    let dir = exe_dir.join("data").join("subject").join("english");
    let db_file = dir.join("note_db.db");
    println!("[Desktop] note db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

/// 📱手机端Android：app_data_dir/subject/english/note_db.db
#[cfg(mobile)]
pub fn get_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = data_dir.join("subject").join("english");
    let db_file = dir.join("note_db.db");
    println!("[Mobile] note db path: {:?}", db_file);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(db_file)
}

// 建表：folder_table + note_table（启用外键，删目录级联删文件）
fn init_tables(conn: &Connection) -> Result<(), String> {
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS folder_table (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            deletable INTEGER NOT NULL DEFAULT 1,
            renamable INTEGER NOT NULL DEFAULT 1
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS note_table (
            id TEXT PRIMARY KEY,
            folder_id TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            date TEXT NOT NULL DEFAULT '',
            content TEXT NOT NULL DEFAULT '',
            FOREIGN KEY (folder_id) REFERENCES folder_table(id) ON DELETE CASCADE
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    // 兼容旧库（曾短暂把正文拆到 note_content_table 的版本）：
    //   - 若 note_table 缺少 content 列，补回该列，避免 INSERT/UPDATE 报“无此列”
    //   - 清理废弃的 note_content_table，避免歧义
    let has_content_col: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('note_table') WHERE name = 'content'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if has_content_col == 0 {
        conn.execute(
            "ALTER TABLE note_table ADD COLUMN content TEXT NOT NULL DEFAULT ''",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    conn.execute("DROP TABLE IF EXISTS note_content_table", [])
        .ok();

    Ok(())
}

// 打开连接并建表（每个命令复用）
fn open_db(app: &tauri::AppHandle) -> Result<Connection, String> {
    let db_path = get_db_path(app)?;
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    init_tables(&conn)?;
    Ok(conn)
}

// 当前 UTC 日期字符串（YYYY-MM-DD），用于新建文件的默认日期（零新增依赖）
fn today_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    const DAY: u64 = 86400;
    let days = (secs / DAY) as i64 + 719468; // 自 1970-01-01 起的天数 + 偏移
    let z = days;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

// 加载笔记树（目录 + 目录下的文件「元数据」，不含正文 content，列表加载更快、更省内存）
#[tauri::command(rename_all = "snake_case")]
pub fn load_note_tree(app: tauri::AppHandle) -> Result<Vec<serde_json::Value>, String> {
    let conn = open_db(&app)?;

    // 若没有任何目录，先种子化默认目录（不可删 / 不可改名）
    let folder_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folder_table", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if folder_count == 0 {
        conn.execute(
            "INSERT INTO folder_table (id, name, deletable, renamable) VALUES ('default', '默认目录', 0, 0)",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    let mut fstmt = conn
        .prepare("SELECT id, name, deletable, renamable FROM folder_table ORDER BY rowid ASC")
        .map_err(|e| e.to_string())?;
    let folder_rows = fstmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut folders = Vec::new();
    for fr in folder_rows {
        let (id, name, deletable, renamable) = fr.map_err(|e| e.to_string())?;

        // 只取元数据（不含 content），正文由 load_note_content 按需读取
        let mut nstmt = conn
            .prepare(
                "SELECT id, folder_id, title, date FROM note_table WHERE folder_id = ?1 ORDER BY rowid ASC",
            )
            .map_err(|e| e.to_string())?;
        let note_rows = nstmt
            .query_map(params![id.clone()], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "folder_id": row.get::<_, String>(1)?,
                    "title": row.get::<_, String>(2)?,
                    "date": row.get::<_, String>(3)?,
                    "content": "",
                }))
            })
            .map_err(|e| e.to_string())?;
        let mut notes = Vec::new();
        for nr in note_rows {
            notes.push(nr.map_err(|e| e.to_string())?);
        }

        folders.push(serde_json::json!({
            "id": id,
            "name": name,
            "deletable": deletable != 0,
            "renamable": renamable != 0,
            "expanded": true,
            "notes": notes,
        }));
    }

    Ok(folders)
}

// 按需加载某篇文件的正文（点击/打开文件时调用；无内容返回空串）
#[tauri::command(rename_all = "snake_case")]
pub fn load_note_content(
    app: tauri::AppHandle,
    note_id: String,
) -> Result<String, String> {
    let conn = open_db(&app)?;
    let content: String = conn
        .query_row(
            "SELECT content FROM note_table WHERE id = ?1",
            params![note_id],
            |row| row.get(0),
        )
        .unwrap_or_default();
    Ok(content)
}

// 新建目录：后端生成 id（f + UUID）并写库，返回新 id 供前端更新 UI 状态
// （前端只负责 UI 树/选中，ID 与落库均由后端完成）
#[tauri::command(rename_all = "snake_case")]
pub fn create_folder(app: tauri::AppHandle, name: String) -> Result<String, String> {
    let conn = open_db(&app)?;
    let id = format!("f{}", Uuid::new_v4().simple());
    conn.execute(
        "INSERT INTO folder_table (id, name, deletable, renamable) VALUES (?1, ?2, 1, 1)",
        params![id, name],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

// 新建文件：后端生成 id（n + UUID）并写库，同时落库 title（由前端传入），
// 返回新 id 供前端更新 UI 状态。
// ⚠️ 必须与 create_folder 对齐——title 必须在创建时落库，不能只写内存 UI 树，
// 否则目录树一旦 reload（切页/导入刷新/重启）就会丢失标题（曾踩坑：新建文件名消失）。
#[tauri::command(rename_all = "snake_case")]
pub fn create_note(
    app: tauri::AppHandle,
    folder_id: String,
    title: String,
) -> Result<String, String> {
    let conn = open_db(&app)?;
    let id = format!("n{}", Uuid::new_v4().simple());
    let date = today_string();
    let title = title.trim().to_string();
    conn.execute(
        "INSERT INTO note_table (id, folder_id, title, date, content) VALUES (?1, ?2, ?3, ?4, '')",
        params![id, folder_id, title, date],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

// 保存目录（upsert：改名复用，按 id 冲突覆盖）
#[tauri::command(rename_all = "snake_case")]
pub fn save_folder(
    app: tauri::AppHandle,
    id: String,
    name: String,
    deletable: i64,
    renamable: i64,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute(
        "INSERT INTO folder_table (id, name, deletable, renamable) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, deletable = excluded.deletable, renamable = excluded.renamable",
        params![id, name, deletable, renamable],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// 保存文件（仅改名 / 移动：更新 title / date / folder_id，绝不触碰 content，
// 避免改名时把正文清空；正文由 update_note_content 单独写入。
// 参数名必须 snake_case：folder_id / _content(保留以兼容调用，不使用)）
#[tauri::command(rename_all = "snake_case")]
pub fn save_note(
    app: tauri::AppHandle,
    id: String,
    folder_id: String,
    title: String,
    date: String,
    _content: String,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute(
        "INSERT INTO note_table (id, folder_id, title, date, content) VALUES (?1, ?2, ?3, ?4, '')
         ON CONFLICT(id) DO UPDATE SET folder_id = excluded.folder_id, title = excluded.title, date = excluded.date",
        params![id, folder_id, title, date],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// 删除目录（级联删除其下所有文件）
#[tauri::command(rename_all = "snake_case")]
pub fn delete_folder(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM folder_table WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// 删除单个文件
#[tauri::command(rename_all = "snake_case")]
pub fn delete_note(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute("DELETE FROM note_table WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// 移动单个文件到目标目录（参数名必须 snake_case：folder_id）
#[tauri::command(rename_all = "snake_case")]
pub fn move_note(
    app: tauri::AppHandle,
    id: String,
    folder_id: String,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute(
        "UPDATE note_table SET folder_id = ?2 WHERE id = ?1",
        params![id, folder_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// 批量移动多个文件到目标目录（事务；参数名必须 snake_case：note_ids / folder_id）
#[tauri::command(rename_all = "snake_case")]
pub fn batch_move_notes(
    app: tauri::AppHandle,
    note_ids: Vec<String>,
    folder_id: String,
) -> Result<(), String> {
    let mut conn = open_db(&app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for nid in &note_ids {
        tx.execute(
            "UPDATE note_table SET folder_id = ?2 WHERE id = ?1",
            params![nid, folder_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// 批量删除多个文件（事务；参数名必须 snake_case：note_ids）
#[tauri::command(rename_all = "snake_case")]
pub fn batch_delete_notes(
    app: tauri::AppHandle,
    note_ids: Vec<String>,
) -> Result<(), String> {
    let mut conn = open_db(&app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for nid in &note_ids {
        tx.execute("DELETE FROM note_table WHERE id = ?1", params![nid])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// 批量删除多个目录（单事务、单开库；级联删其下所有文件）
// ⚠️ 用于取代前端「逐个 delete_folder」的 N 次开库：把 1+N 次 open_db 降到 1 次。
// 参数名必须 snake_case：folder_ids
#[tauri::command(rename_all = "snake_case")]
pub fn batch_delete_folders(
    app: tauri::AppHandle,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    let mut conn = open_db(&app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for fid in &folder_ids {
        tx.execute("DELETE FROM folder_table WHERE id = ?1", params![fid])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// 单独保存文件富文本内容（参数名必须 snake_case：id / content）
#[tauri::command(rename_all = "snake_case")]
pub fn update_note_content(
    app: tauri::AppHandle,
    id: String,
    content: String,
) -> Result<(), String> {
    let conn = open_db(&app)?;
    conn.execute(
        "UPDATE note_table SET content = ?2 WHERE id = ?1",
        params![id, content],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================
// 导入：从 zip 内读出的 note_db 字节，合并进当前笔记库
//   - 不做查重
//   - 导入的目录统一重命名为「导入目录-原目录名称」，并分配全新 id（避免与现有目录冲突）
//   - 笔记也分配全新 id，folder_id 映射到重命名后的新目录
//   - 导入的目录强制可删除 / 可改名（deletable=1, renamable=1）
// 由 lib.rs 的 import_subject_zip 按 manifest 的 module="note" 派发调用
// ============================================================
pub fn import_note_bytes(
    bytes: &[u8],
    app: &tauri::AppHandle,
    prefix: &str,
    imported: &mut i64,
    skipped: &mut i64,
) -> Result<(), String> {
    let cache = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let tmp = cache.join(format!("note_import_{}.db", Uuid::new_v4().simple()));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;

    let src = Connection::open(&tmp).map_err(|e| e.to_string())?;

    // 必须同时具备 folder_table 与 note_table 才视为合法的笔记库
    let has_folder: i64 = src
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='folder_table')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let has_note: i64 = src
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='note_table')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if has_folder == 0 || has_note == 0 {
        let _ = fs::remove_file(&tmp);
        *skipped += 1;
        return Ok(());
    }

    // 读源库：目录 + 文件
    let mut folder_rows = src
        .prepare("SELECT id, name, deletable, renamable FROM folder_table ORDER BY rowid ASC")
        .map_err(|e| e.to_string())?;
    let folders = folder_rows
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut folder_list: Vec<(String, String, i64, i64)> = Vec::new();
    for f in folders {
        folder_list.push(f.map_err(|e| e.to_string())?);
    }

    let mut note_rows = src
        .prepare("SELECT id, folder_id, title, date, content FROM note_table ORDER BY rowid ASC")
        .map_err(|e| e.to_string())?;
    let notes = note_rows
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut note_list: Vec<(String, String, String, String, String)> = Vec::new();
    for n in notes {
        note_list.push(n.map_err(|e| e.to_string())?);
    }

    // 合并进当前笔记库（事务）
    let mut dst = open_db(app)?;
    let tx = dst.transaction().map_err(|e| e.to_string())?;

    // 目录：重命名 + 新 id + 强制可删/可改名
    let mut folder_map: HashMap<String, String> = HashMap::new();
    for (old_id, name, _d, _r) in &folder_list {
        let new_id = format!("f{}", Uuid::new_v4().simple());
        let new_name = format!("{}-{}", prefix, name);
        folder_map.insert(old_id.clone(), new_id.clone());
        tx.execute(
            "INSERT INTO folder_table (id, name, deletable, renamable) VALUES (?1,?2,1,1)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, deletable = 1, renamable = 1",
            params![new_id, new_name],
        )
        .map_err(|e| e.to_string())?;
        *imported += 1;
    }

    // 文件：新 id + 映射到重命名后的目录
    for (_old_id, old_folder, title, date, content) in &note_list {
        let new_id = format!("n{}", Uuid::new_v4().simple());
        let new_folder = match folder_map.get(old_folder) {
            Some(f) => f.clone(),
            None => continue, // 笔记指向了不存在的目录，跳过该笔记
        };
        tx.execute(
            "INSERT INTO note_table (id, folder_id, title, date, content) VALUES (?1,?2,?3,?4,?5)",
            params![new_id, new_folder, title, date, content],
        )
        .map_err(|e| e.to_string())?;
        *imported += 1;
    }

    tx.commit().map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&tmp);
    Ok(())
}
