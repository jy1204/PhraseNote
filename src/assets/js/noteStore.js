// ============================================================
// noteStore.js —— 笔记数据层（前端）
// 负责：从 Tauri 后端 note_db.db 加载目录/文件树，并把所有增删改操作持久化到数据库。
// 仿照 PhrasePage 的调用方式（invoke 走 @tauri-apps/api/core）。
// 为方便浏览器 dev（非 Tauri 环境），保留一份内存兜底数据，确保页面不会空白。
// ============================================================

import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

// 是否运行在 Tauri 环境：浏览器 dev 下无 __TAURI_INTERNALS__，走内存兜底
const isTauri =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

// 共享的目录树（NotePage 与 NoteEditor 共用，selectedNote 也指向其中的文件对象）
export const folders = ref([])

// 浏览器 dev 兜底数据（仅非 Tauri 环境下使用，避免页面空白；与后端种子化默认目录一致）
const FALLBACK = [
  { id: 'default', name: '默认目录', deletable: false, renamable: false, expanded: true, notes: [] },
  {
    id: 'f1', name: '英语笔记', deletable: true, renamable: true, expanded: false, notes: [
      { id: 'n101', title: '定语从句总结', date: '2026-10-15', content: '' },
      { id: 'n102', title: '虚拟语气笔记', date: '2026-10-14', content: '' },
      { id: 'n103', title: '易错介词', date: '2026-10-12', content: '' },
      { id: 'n104', title: '长难句拆解', date: '2026-10-10', content: '' },
    ],
  },
  {
    id: 'f2', name: '旅行计划', deletable: true, renamable: true, expanded: false, notes: [
      { id: 'n201', title: '东京行程 Day1', date: '2026-10-12', content: '' },
      { id: 'n202', title: '交通卡对比', date: '2026-10-11', content: '' },
      { id: 'n203', title: '预算草稿', date: '2026-10-09', content: '' },
    ],
  },
  {
    id: 'f3', name: '读书摘录', deletable: true, renamable: true, expanded: false, notes: [
      { id: 'n301', title: '《人类简史》', date: '2026-10-08', content: '' },
      { id: 'n302', title: '《活着》', date: '2026-10-06', content: '' },
      { id: 'n303', title: '《三体》', date: '2026-10-03', content: '' },
      { id: 'n304', title: '《小王子》', date: '2026-09-30', content: '' },
    ],
  },
]

// 加载目录树（从数据库；非 Tauri 用兜底数据）
export async function loadFolders() {
  if (isTauri) {
    try {
      const tree = await invoke('load_note_tree')
      folders.value = Array.isArray(tree) ? tree : []
    } catch (e) {
      console.error('[note] load_note_tree 失败：', e)
      folders.value = []
    }
    return
  }
  if (folders.value.length === 0) {
    folders.value = JSON.parse(JSON.stringify(FALLBACK))
  }
}

// ---------- 持久化封装（非 Tauri 环境直接跳过，仅操作内存） ----------

// 新建目录：交由后端生成 id 并写库，返回新 id（前端只用于更新 UI 树）
// 非 Tauri（浏览器 dev）兜底：本地生成占位 id，保证页面可用
export async function createFolderDb(name) {
  if (isTauri) {
    try {
      return await invoke('create_folder', { name })
    } catch (e) {
      console.error('[note] create_folder 失败：', e)
      return null
    }
  }
  return 'f' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
}

// 新建文件：交由后端生成 id 并写库（同时落库 title），返回新 id（前端只用于更新 UI 树）
// 非 Tauri（浏览器 dev）兜底：本地生成占位 id，保证页面可用
export async function createNoteDb(folderId, title = '') {
  if (isTauri) {
    try {
      return await invoke('create_note', { folder_id: folderId, title })
    } catch (e) {
      console.error('[note] create_note 失败：', e)
      return null
    }
  }
  return 'n' + Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
}

export async function saveFolder(folder) {
  if (!isTauri) return
  await invoke('save_folder', {
    id: folder.id,
    name: folder.name,
    deletable: folder.deletable ? 1 : 0,
    renamable: folder.renamable ? 1 : 0,
  })
}

// 保存文件（upsert：新增 / 改名 / 移动 / 改内容；参数名必须 snake_case：folder_id）
export async function saveNote(note, folderId) {
  if (!isTauri) return
  await invoke('save_note', {
    id: note.id,
    folder_id: folderId,
    title: note.title || '',
    date: note.date || '',
    content: note.content || '',
  })
}

// 按需加载某篇文件的正文（点击文件打开编辑器时调用；参数名必须 snake_case：note_id）
export async function loadNoteContent(noteId) {
  if (!isTauri) {
    // 非 Tauri（浏览器 dev）兜底：内存里找
    for (const f of folders.value) {
      const n = f.notes.find((x) => x.id === noteId)
      if (n) return n.content || ''
    }
    return ''
  }
  try {
    const c = await invoke('load_note_content', { note_id: noteId })
    return typeof c === 'string' ? c : ''
  } catch (e) {
    console.error('[note] load_note_content 失败：', e)
    return ''
  }
}

// 删除目录（级联删其下文件）
export async function deleteFolderDb(id) {
  if (!isTauri) return
  await invoke('delete_folder', { id })
}

export async function deleteNoteDb(id) {
  if (!isTauri) return
  await invoke('delete_note', { id })
}

export async function moveNoteDb(id, folderId) {
  if (!isTauri) return
  await invoke('move_note', { id, folder_id: folderId })
}

export async function batchMoveDb(noteIds, folderId) {
  if (!isTauri) return
  await invoke('batch_move_notes', { note_ids: noteIds, folder_id: folderId })
}

export async function batchDeleteNotesDb(noteIds) {
  if (!isTauri) return
  await invoke('batch_delete_notes', { note_ids: noteIds })
}

// 批量删除目录（单事务、单开库；级联删其下文件）—— 取代前端逐个 delete_folder
export async function batchDeleteFoldersDb(folderIds) {
  if (!isTauri) return
  await invoke('batch_delete_folders', { folder_ids: folderIds })
}

export async function updateNoteContentDb(id, content) {
  if (!isTauri) return
  await invoke('update_note_content', { id, content })
}
