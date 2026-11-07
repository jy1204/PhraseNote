<template>
  <div class="note-page">
    <div class="note-body">
    <!-- 左侧竖排操作条：添加目录 + 管理（视觉风格对齐 PhrasePage 的 .type 竖条，管理功能后续接入） -->
    <div class="note-side">
      <div class="side-item side-add" @click="addFolder" title="添加目录">
        <el-icon><FolderAdd /></el-icon>
      </div>
      <div class="side-item side-manage" :class="{ active: manageMode }" title="管理" @click="toggleManage">
        <el-icon><Operation /></el-icon>
      </div>
    </div>

    <!-- 右侧主区：搜索框 + 收藏夹树 -->
    <div class="note-main">
      <div class="tools">
        <div class="search-title pn-search">
          <el-input
            ref="search_note_input"
            v-model="search_note_input_content"
            class="search-input"
            placeholder="搜索标题或目录"
            :prefix-icon="Search"
          />
          <button type="button" class="search-input-clear" @click="clear_search_input">
            <el-icon><CircleCloseFilled /></el-icon>
          </button>
        </div>
      </div>

      <!-- 列表视图：收藏夹树（点击文件后由 App 整屏跳转到编辑视图） -->
      <div class="note-list-view" :class="{ 'has-more': treeHasMoreBelow }">
      <div class="note-tree" ref="treeRef" @scroll="updateTreeFade">
        <div v-if="filteredFolders.length === 0" class="tree-empty">未找到匹配的笔记</div>
        <div class="tree-folder" v-for="folder in filteredFolders" :key="folder.id">
          <div class="tree-folder-header" @click="manageMode ? toggleFolderSelect(folder) : toggleFolder(folder)">
            <span v-if="manageMode" class="tree-check" :class="folderCheckState(folder)" @click.stop="toggleFolderSelect(folder)">
              <el-icon v-if="folderCheckState(folder) === 'checked'"><Check /></el-icon>
              <span v-else-if="folderCheckState(folder) === 'indeterminate'" class="dash"></span>
            </span>
            <span class="tree-arrow" :class="{ expanded: folder.expanded }" @click.stop="toggleFolder(folder)">
              <el-icon><ArrowRight /></el-icon>
            </span>
            <span class="tree-folder-icon"><el-icon><Folder /></el-icon></span>
            <span class="tree-folder-name" v-marquee>
              <span class="marquee-chunk">{{ folder.name }}</span>
              <span class="marquee-chunk" aria-hidden="true">{{ folder.name }}</span>
            </span>
            <span class="tree-folder-actions" v-show="!manageMode" @click.stop>
              <el-icon class="act" title="删除文件夹" @click="deleteFolder(folder)"><Delete /></el-icon>
              <el-icon class="act" title="重命名" @click="startRename('folder', folder)"><EditPen /></el-icon>
              <el-icon class="act act-add" title="新建文件" @click="addNote(folder)"><Document /></el-icon>
            </span>
          </div>
          <div class="tree-children" v-show="folder.expanded">
            <div
              class="tree-note"
              v-for="note in folder.notes"
              :key="note.id"
              @click="manageMode ? toggleNoteSelect(folder, note) : selectNote(note.id)"
            >
              <span v-if="manageMode" class="tree-check" :class="{ checked: isNoteChecked(note) }" @click.stop="toggleNoteSelect(folder, note)">
                <el-icon v-if="isNoteChecked(note)"><Check /></el-icon>
              </span>
              <span class="tree-note-icon"><el-icon><Document /></el-icon></span>
              <span class="tree-note-body">
                <span class="tree-note-title" v-marquee>
                  <span class="marquee-chunk">{{ note.title }}</span>
                  <span class="marquee-chunk" aria-hidden="true">{{ note.title }}</span>
                </span>
                <span class="tree-note-meta">{{ note.date }}</span>
              </span>
              <span class="tree-note-actions" v-show="!manageMode" @click.stop>
                <el-icon class="act act-del" title="删除文件" @click="deleteNote(folder, note)"><Delete /></el-icon>
                <el-icon class="act" title="重命名" @click="startRename('note', note)"><EditPen /></el-icon>
                <el-icon class="act" title="移动至文件夹" @click="openMove(note, folder.id)"><Rank /></el-icon>
              </span>
            </div>
            <div v-if="folder.notes.length === 0" class="tree-empty">暂无文件</div>
          </div>
        </div>
      </div>
    </div>

    <!-- 移动至文件夹：选择目标收藏夹 -->
    <div v-if="moveDialog.visible" class="move-mask" @click.self="moveDialog.visible = false">
      <div class="move-dialog">
        <h2>移动到</h2>
        <div class="move-list">
          <div
            v-for="f in folders"
            :key="f.id"
            class="move-item"
            :class="{ disabled: f.id === moveDialog.fromFolderId }"
            @click="f.id !== moveDialog.fromFolderId && doMove(f.id)"
          >{{ f.name }}</div>
        </div>
        <div class="move-actions">
          <el-icon class="act act-add" title="新建目录" @click="addFolder"><FolderAdd /></el-icon>
          <el-button @click="moveDialog.visible = false">取消</el-button>
        </div>
      </div>
    </div>
    </div>
    </div>

    <!-- 管理模式底部操作栏：移动 / 删除 / 取消 + 全选 + 计数（位于 note-page 下，横跨整个底部） -->
    <div class="manage-bar" v-if="manageMode">
      <div class="manage-info">
        <span class="manage-select-all" @click="toggleSelectAll">{{ allSelected ? '全不选' : '全选' }}</span>
        <span class="manage-count">已选 {{ selectedCount }} 项</span>
      </div>
      <div class="manage-actions">
        <button class="mbtn mbtn-move" :disabled="!hasSelection" @click="batchMove">移动</button>
        <button class="mbtn mbtn-del" :disabled="!hasSelection" @click="batchDelete">删除</button>
        <button class="mbtn mbtn-cancel" @click="toggleManage">取消</button>
      </div>
    </div>
  </div>
</template>

<script setup>
  // NotePage 的逻辑（富文本编辑器 + 搜索框 + 平台样式加载）抽到独立的组合式函数
  import { ref, computed, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'
  import {
    Search, ArrowRight, Folder, FolderAdd, Document,
    Delete, EditPen, Rank, CircleCloseFilled, Operation, Check,
  } from '@element-plus/icons-vue'
  import { useNotePage, vMarquee } from '../js/NotePage.js'
  // NotePage 公共样式：随本组件加载（始终加载）
  import '../css/public/NotePage.css'
  import { ElMessage, ElMessageBox } from 'element-plus'
  import { openNote, closeNote, manageMode, checkedFolders, checkedNotes, enterManage, exitManage, noteDataVersion } from '../js/index.js'
  // 笔记数据层：目录树 + 持久化（note_db.db）
  // 前端只负责 UI 树/选中状态；新建/保存/删除/移动均交由后端完成（ID 也由后端生成）
  import {
    folders, loadFolders,
    createFolderDb, createNoteDb,
    saveFolder, saveNote, deleteFolderDb, deleteNoteDb,
    moveNoteDb, batchMoveDb, batchDeleteNotesDb, batchDeleteFoldersDb,
  } from '../js/noteStore.js'

  // 搜索框状态：本地 ref，确保模板 v-model 绑定为明确的响应式
  // （规避跨文件解构 ref 在 <script setup> 中的绑定隐患）
  const search_note_input = ref(null)
  const search_note_input_content = ref('')
  function clear_search_input() {
    search_note_input_content.value = ''
    // 清空后自动聚焦到输入框（el-input 暴露 focus()，用 nextTick 确保 DOM 已更新）
    nextTick(() => {
      const el = search_note_input.value
      if (el && typeof el.focus === 'function') el.focus()
    })
  }
  // 底部渐隐遮罩（纯视觉）：滚动条已全局隐藏，用它提示目录树下方还有内容，
  // 滚到底自动淡出（CSS 由 .note-list-view.has-more 控制）
  const treeRef = ref(null)
  const treeHasMoreBelow = ref(false)
  function updateTreeFade() {
    const el = treeRef.value
    if (!el) return
    treeHasMoreBelow.value = el.scrollTop + el.clientHeight < el.scrollHeight - 8
  }
  // 树的展开/收起、增删文件都会改变可滚动高度，深监听后重算（笔记数据量小，开销可忽略）
  watch(folders, () => { nextTick(updateTreeFade) }, { deep: true })

  // 保留 useNotePage() 调用：其内部按平台动态加载 mobile / desktop 样式
  useNotePage()

  // ---------- 树形数据：folders 嵌套 notes ----------
  // folders 来自 noteStore.js（共享 ref，NoteEditor 也引用其中的文件对象）
  // onMounted 时从 note_db.db 加载；非 Tauri 环境自动走内存兜底数据
  function today() {
    const d = new Date()
    const p = (n) => String(n).padStart(2, '0')
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
  }

  onMounted(() => {
    loadFolders()
  })

  // 离开本页（切到其它页面）时复位管理模式：manageMode / 勾选集都是 index.js 里的
  // 全局单例，不会随组件卸载自动消失；若不在此重置，下次回到本页仍会停在管理态并带着旧勾选。
  // 历史栈无需处理——切页面走 router.replace，enterManage 压入的 { pnManage } 记录已被新路由替换。
  onBeforeUnmount(() => {
    exitManage()
  })

  // 导入（zip 备份）成功后，App 会通过 noteDataVersion 自增触发本页刷新（强制重读目录树）
  // 笔记与短语分开维护版本号：只导入短语时不会误刷笔记，只导入笔记时也不会漏刷
  watch(noteDataVersion, () => {
    loadFolders()
  })

  // 搜索：按笔记标题或文件夹名实时过滤；命中时文件夹自动展开
  // 文件夹名命中 → 展示整个文件夹（含其全部笔记）；否则只展示标题命中的笔记。
  // 文件夹名命中时即使该文件夹为空也保留，确保“搜目录名能找到对应目录”。
  const searchKeyword = computed(() => (search_note_input_content.value || '').trim().toLowerCase())
  const filteredFolders = computed(() => {
    const kw = searchKeyword.value
    if (!kw) return folders.value
    const result = []
    for (const f of folders.value) {
      const nameHit = f.name.toLowerCase().includes(kw)
      if (nameHit) {
        // 目录名命中：保留整个目录（含全部笔记）
        result.push({ ...f, notes: f.notes, expanded: true })
        continue
      }
      const matchedNotes = f.notes.filter((n) => (n.title || '').toLowerCase().includes(kw))
      if (matchedNotes.length > 0) {
        result.push({ ...f, notes: matchedNotes, expanded: true })
      }
    }
    return result
  })

  // ---------- 管理模式（批量移动 / 删除） ----------
  // manageMode / checkedFolders / checkedNotes 已提升为共享状态（见 index.js），供安卓返回键拦截使用

  function toggleManage() {
    if (manageMode.value) {
      // 退出（按钮路径）：取消管理模式并回退 enterManage 压入的历史，保持栈平衡
      exitManage()
      if (typeof window !== 'undefined' && window.history) window.history.back()
    } else {
      enterManage()
    }
  }

  // 文件夹勾选态：checked（全选）/ indeterminate（部分）/ unchecked（无）
  function folderCheckState(folder) {
    if (checkedFolders.value.has(folder.id)) return 'checked'
    const total = folder.notes.length
    if (total === 0) return 'unchecked'
    const sel = folder.notes.filter((n) => checkedNotes.value.has(n.id)).length
    if (sel === 0) return 'unchecked'
    return sel === total ? 'checked' : 'indeterminate'
  }
  function isNoteChecked(note) { return checkedNotes.value.has(note.id) }

  // 勾选 / 取消文件夹：级联其下所有文件
  function toggleFolderSelect(folder) {
    if (checkedFolders.value.has(folder.id)) {
      checkedFolders.value.delete(folder.id)
      folder.notes.forEach((n) => checkedNotes.value.delete(n.id))
    } else {
      checkedFolders.value.add(folder.id)
      folder.notes.forEach((n) => checkedNotes.value.add(n.id))
    }
  }
  // 勾选 / 取消单个文件：影响所属文件夹的勾选态
  function toggleNoteSelect(folder, note) {
    if (checkedNotes.value.has(note.id)) {
      checkedNotes.value.delete(note.id)
      if (checkedFolders.value.has(folder.id)) checkedFolders.value.delete(folder.id)
    } else {
      checkedNotes.value.add(note.id)
      if (folder.notes.every((n) => checkedNotes.value.has(n.id))) {
        checkedFolders.value.add(folder.id)
      }
    }
  }

  // 实际被选中的文件（去重，按文件粒度）：整文件夹选中 + 单独选中的文件合并
  function effectiveSelectedNotes() {
    const result = []
    const seen = new Set()
    for (const f of folders.value) {
      const folderChecked = checkedFolders.value.has(f.id)
      for (const n of f.notes) {
        if (folderChecked || checkedNotes.value.has(n.id)) {
          if (!seen.has(n.id)) { seen.add(n.id); result.push({ note: n, folder: f }) }
        }
      }
    }
    return result
  }

  // 选中总数（仅统计笔记，目录不计入计数；遵循“目录不想计数”的约定）
  // “是否有任何选中”单独用 hasSelection 表达（整选目录也算），供按钮可用性判断
  const selectedNoteCount = computed(() => effectiveSelectedNotes().length)
  const selectedFolderCount = computed(() => checkedFolders.value.size)
  const selectedCount = computed(() => selectedNoteCount.value)
  const hasSelection = computed(() => selectedNoteCount.value > 0 || selectedFolderCount.value > 0)
  const totalFolders = computed(() => folders.value.length)
  const totalNotes = computed(() => folders.value.reduce((s, f) => s + f.notes.length, 0))
  const allSelected = computed(() => {
    const allFolders = totalFolders.value > 0 && checkedFolders.value.size === totalFolders.value
    const allNotes = totalNotes.value === 0 || selectedNoteCount.value === totalNotes.value
    return allFolders && allNotes
  })
  function toggleSelectAll() {
    if (allSelected.value) {
      checkedFolders.value.clear(); checkedNotes.value.clear()
    } else {
      folders.value.forEach((f) => {
        checkedFolders.value.add(f.id)
        f.notes.forEach((n) => checkedNotes.value.add(n.id))
      })
    }
  }

  // 批量移动：复用“移动到”弹窗，传入选中数组
  // 移动只对“含笔记的目录/笔记”有意义：空目录（无论是否整选）没有笔记可搬，
  // 只支持删除（见 batchDelete），此处不弹移动框，直接提示“没有可移动的笔记”。
  function batchMove() {
    if (!hasSelection.value) return
    if (selectedNoteCount.value === 0) {
      ElMessage({ message: '没有可移动的笔记', type: 'info' , offset: 50, duration: 3000})
      return
    }
    moveDialog.value = { visible: true, noteId: null, fromFolderId: null, batch: true }
  }
  // 批量删除：默认目录不可删（仅清空其被选中的文件），其余整文件夹选中则删文件夹
  // 目录即使为空（内部无文件）也要能删除——之前因只统计笔记而漏掉了空目录
  async function batchDelete() {
    if (!hasSelection.value) return
    const items = effectiveSelectedNotes()
    // 被整选且可删的目录（含空目录）
    const foldersToDelete = folders.value.filter((f) => checkedFolders.value.has(f.id) && f.deletable)
    if (items.length === 0 && foldersToDelete.length === 0) return
    const noteCount = items.length
    const folderCount = foldersToDelete.length
    let msg
    if (noteCount > 0 && folderCount > 0) msg = `确定删除选中的 ${noteCount} 篇笔记和 ${folderCount} 个目录？`
    else if (noteCount > 0) msg = `确定删除选中的 ${noteCount} 篇笔记？`
    else msg = `确定删除选中的 ${folderCount} 个目录？`
    try {
      await ElMessageBox.confirm(msg, '删除确认', {
        type: 'warning', confirmButtonText: '确认', cancelButtonText: '取消',
      })
    } catch (e) { return }
    const noteIds = []
    const folderIdsToDelete = foldersToDelete.map((f) => f.id)
    const folderIdSet = new Set(folderIdsToDelete)
    for (const { note, folder } of items) {
      // 整选的目录会被级联删除，其下的笔记无需单独删除，跳过以省一次写库
      if (folderIdSet.has(folder.id)) continue
      const i = folder.notes.findIndex((n) => n.id === note.id)
      if (i >= 0) { folder.notes.splice(i, 1); noteIds.push(note.id) }
    }
    // 从 UI 树移除整选的可删目录（默认目录不删，仅清空其文件）
    for (const f of foldersToDelete) {
      const idx = folders.value.findIndex((x) => x.id === f.id)
      if (idx >= 0) folders.value.splice(idx, 1)
    }
    // 持久化：批量删文件（1 次开库）+ 批量删目录（1 次开库，单事务），取代原先 1+N 次开库
    if (noteIds.length > 0) await batchDeleteNotesDb(noteIds)
    if (folderIdsToDelete.length > 0) await batchDeleteFoldersDb(folderIdsToDelete)
    checkedFolders.value.clear(); checkedNotes.value.clear()
    ElMessage({ message: `已删除 ${noteIds.length} 篇、${foldersToDelete.length} 个目录`, type: 'success', offset: 50, duration: 3000 })
  }

  // 新建文件夹（＋按钮：先弹命名提示，确认后添加；取消则不创建）
  function addFolder() {
    ElMessageBox.prompt('请输入目录名称', '新建目录', {
      inputValue: '新建目录',
      inputPattern: /\S+/,
      inputErrorMessage: '名称不能为空',
      type: 'info',
      confirmButtonText: '确认',
      cancelButtonText: '取消',
    })
      .then(async ({ value }) => {
        const name = (value || '').trim() || '新建目录'
        // ID 由后端生成并返回；前端仅用其更新 UI 树（不自己造 ID、不直接写库）
        const id = await createFolderDb(name)
        if (!id) return
        folders.value.push({ id, name, deletable: true, renamable: true, expanded: true, notes: [] })
      })
      .catch(() => {})
  }

  // 在某文件夹下新建文件：先弹命名提示，确认后创建（自动展开并显示）；取消则不创建
  function addNote(folder) {
    ElMessageBox.prompt('请输入文件名称', '新建文件', {
      inputValue: '未命名文件',
      inputPattern: /\S+/,
      inputErrorMessage: '名称不能为空',
      type: 'info',
      confirmButtonText: '确认',
      cancelButtonText: '取消',
    })
      .then(async ({ value }) => {
        const title = (value || '').trim() || '未命名文件'
        // ID 由后端生成并返回；后端同时落库 title（避免 reload 后标题丢失）
        const id = await createNoteDb(folder.id, title)
        if (!id) return
        const note = { id, title, date: today(), content: '' }
        folder.notes.push(note)
        folder.expanded = true
      })
      .catch(() => {})
  }

  // 展开 / 收起
  function toggleFolder(folder) { folder.expanded = !folder.expanded }

  // 选中文件：写入共享状态并压入历史栈，App 据此整屏跳转到编辑视图
  function selectNote(id) {
    if (id == null) { closeNote(); return }
    for (const f of folders.value) {
      const n = f.notes.find((x) => x.id === id)
      if (n) { openNote(n); return }
    }
  }

  // 删除文件夹：默认目录拦截 + 确认
  function deleteFolder(folder) {
    if (!folder.deletable) {
      ElMessage({ message: '默认目录不可删除', type: 'warning', offset: 50, duration: 3000 })
      return
    }
    ElMessageBox.confirm(`确定删除文件夹“${folder.name}”及其下所有笔记？`, '删除确认', { type: 'warning', confirmButtonText: '确认', cancelButtonText: '取消' })
      .then(async () => {
        const i = folders.value.findIndex((f) => f.id === folder.id)
        if (i >= 0) folders.value.splice(i, 1)
        await deleteFolderDb(folder.id)
        ElMessage({ message: '已删除', type: 'success', offset: 50, duration: 3000 })
      })
      .catch(() => {})
  }

  // 重命名（弹窗输入）：默认目录拦截
  function startRename(type, item) {
    if (type === 'folder' && !item.renamable) {
      ElMessage({ message: '默认目录不可改名', type: 'warning' })
      return
    }
    const oldName = type === 'folder' ? item.name : item.title
    ElMessageBox.prompt('请输入新名称', '重命名', { inputValue: oldName, type: 'info', confirmButtonText: '确认', cancelButtonText: '取消' })
      .then(async ({ value }) => {
        const v = (value || '').trim()
        if (!v) return
        if (type === 'folder') {
          const f = folders.value.find((x) => x.id === item.id)
          if (f) { f.name = v; await saveFolder(f) }
        } else {
          for (const f of folders.value) {
            const n = f.notes.find((x) => x.id === item.id)
            if (n) { n.title = v; await saveNote(n, f.id); break }
          }
        }
      })
      .catch(() => {})
  }

  // 删除文件（确认）
  function deleteNote(folder, note) {
    ElMessageBox.confirm(`确定删除笔记“${note.title}”？`, '删除确认', { type: 'warning', confirmButtonText: '确认', cancelButtonText: '取消' })
      .then(async () => {
        const i = folder.notes.findIndex((n) => n.id === note.id)
        if (i >= 0) folder.notes.splice(i, 1)
        await deleteNoteDb(note.id)
        ElMessage({ message: '已删除', type: 'success', offset: 50, duration: 3000})
      })
      .catch(() => {})
  }

  // 移动文件：弹出选择目标收藏夹（单条 or 批量），真实移动
  const moveDialog = ref({ visible: false, noteId: null, fromFolderId: null, batch: false })
  function openMove(note, fromFolderId) {
    moveDialog.value = { visible: true, noteId: note.id, fromFolderId, batch: false }
  }
  function doMove(targetFolderId) {
    const d = moveDialog.value
    const to = folders.value.find((f) => f.id === targetFolderId)
    if (!to) { d.visible = false; return }
    if (!d.batch) {
      const { noteId, fromFolderId } = d
      if (targetFolderId === fromFolderId) { d.visible = false; return }
      const from = folders.value.find((f) => f.id === fromFolderId)
      if (!from) { d.visible = false; return }
      const i = from.notes.findIndex((n) => n.id === noteId)
      if (i >= 0) { const [n] = from.notes.splice(i, 1); to.notes.push(n) }
      d.visible = false
      moveNoteDb(noteId, targetFolderId)
      ElMessage({ message: '已移动', type: 'success', offset: 50, duration: 3000 })
      return
    }
    // 批量：把去重后的选中文件移入目标文件夹，跳过“移入自身所在文件夹”
    const ids = []
    for (const { note, folder } of effectiveSelectedNotes()) {
      if (folder.id === targetFolderId) continue
      const i = folder.notes.findIndex((n) => n.id === note.id)
      if (i >= 0) { const [n] = folder.notes.splice(i, 1); to.notes.push(n); ids.push(note.id) }
    }
    if (ids.length > 0) batchMoveDb(ids, targetFolderId)
    d.visible = false
    checkedFolders.value.clear(); checkedNotes.value.clear()
    ElMessage({
      message: ids.length > 0 ? `已移动 ${ids.length} 篇` : '没有可移动的笔记',
      type: ids.length > 0 ? 'success' : 'info',
      offset: 50,
      duration: 3000
    })
  }
</script>
