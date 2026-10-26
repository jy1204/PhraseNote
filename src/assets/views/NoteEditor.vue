<template>
  <div class="note-editor">
    <!-- 顶部：返回按钮 + 当前笔记标题（整屏编辑，隐藏了原 app 的菜单栏与侧边导航） -->
    <div class="editor-header">
      <div class="editor-back" @click="back">‹</div>
      <div class="editor-title" v-marquee>
        <span class="marquee-chunk">{{ selectedNote?.title || '未命名笔记' }}</span>
        <span class="marquee-chunk" aria-hidden="true">{{ selectedNote?.title || '未命名笔记' }}</span>
      </div>
      <div class="editor-save" @click="onSave" title="保存"><el-icon><Check /></el-icon><span class="save-text">保存</span></div>
    </div>
    <!-- 富文本工具栏 -->
    <Toolbar
      style="border-bottom: 1px solid var(--pn-border, #ccc)"
      :editor="editorRef"
      :defaultConfig="toolbarConfig"
      :mode="mode"
    />
    <!-- 编辑区：占满剩余高度 -->
    <div class="editor-body">
      <Editor
        style="height: 100%; overflow-y: hidden;"
        v-model="valueHtml"
        :defaultConfig="editorConfig"
        :mode="mode"
        @onCreated="handleCreated"
      />
    </div>
  </div>
</template>

<script setup>
  // 全屏笔记编辑视图：选中文件后由 App 整屏渲染，覆盖原 app 的菜单栏与侧边导航
  import { watch } from 'vue'
  import { Editor, Toolbar } from '@wangeditor/editor-for-vue'
  import { Check } from '@element-plus/icons-vue'
  import { ElMessage } from 'element-plus'
  import { useNotePage, vMarquee } from '../js/NotePage.js'
  import { selectedNote, backNote } from '../js/index.js'
  import { updateNoteContentDb, loadNoteContent } from '../js/noteStore.js'
  import '../css/public/NoteEditor.css'

  const {
    editorRef,
    valueHtml,
    mode,
    toolbarConfig,
    editorConfig,
    handleCreated,
  } = useNotePage()

  // 将当前笔记已保存的内容载入编辑器（首次打开 / 切换笔记时调用），并自动聚焦
  // 正文不随列表树加载，而是按 note_id 从后端按需读取，点开文件时才读。
  async function loadContent() {
    const ed = editorRef.value
    if (!ed) return
    const id = selectedNote.value && selectedNote.value.id
    let html = ''
    if (id) {
      try { html = await loadNoteContent(id) } catch (e) { html = '' }
    }
    ed.setHtml(html || '')
    // 等浮层渲染到 DOM、编辑区可见后再聚焦（避免挂载瞬间聚焦失败）
    requestAnimationFrame(() => { ed.focus() })
  }

  // 编辑器实例就绪后载入当前笔记内容
  watch(editorRef, (ed) => { if (ed) loadContent() })
  // 切换笔记（返回历史 / 打开另一篇）时重新载入内容
  watch(selectedNote, () => { if (editorRef.value) loadContent() })

  // 保存：把编辑器内容写回当前笔记对象，并持久化到 note_db.db
  async function onSave() {
    if (!selectedNote.value) return
    const ed = editorRef.value
    const html = ed ? ed.getHtml() : valueHtml.value
    selectedNote.value.content = html
    await updateNoteContentDb(selectedNote.value.id, html)
    ElMessage({ message: '已保存', type: 'success' })
  }

  // 返回：与安卓硬件返回键走同一路径。
  // 若我们为拦截返回键压入了历史记录，则回退该记录（触发 popstate → backNote），
  // 保证历史栈一致；否则（如未压入）直接关掉编辑视图。
  function back() {
    const st =
      typeof window !== 'undefined' && window.history ? window.history.state : null;
    if (st && st.pnEditor) {
      window.history.back();
    } else {
      backNote();
    }
  }
</script>
