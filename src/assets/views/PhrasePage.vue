<template>
  <div class="phrase-page">
    <div class="content-panel">
      <!-- A‑Z / * / # 字母筛选栏 -->
      <div class="type">
        <ol>
          <li v-for="item in typeList" :key="item" :class="{ active: activeChar === item }" @click="handleCharClick(item)">{{ item }}</li>
        </ol>
      </div>
      <!-- 词条列表区域：搜索框始终可见（搜索无结果时也需保留，方便清词/改词），
           空状态提示与词条列表按 show_data 互斥显示 -->
      <div class="content-knowledge" :class="{ 'has-more': listHasMoreBelow }">
        <div class="search-keyword pn-search">
          <el-input ref="search_keyword_input" v-model="search_keyword_input_content" class="search-input" placeholder="搜索关键词" :prefix-icon="Search" />
          <button type="button" class="search-input-clear" @click="clear_search_input">
            <el-icon><CircleCloseFilled /></el-icon>
          </button>
        </div>
        <!-- 空数据提示：没有词条 / 搜索无结果时展示 -->
        <div id="empty-box" v-show="show_data">
          <el-empty :description="emptyDesc">
            <template #image>
              <svg class="empty-illustration" viewBox="0 0 120 100" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
                <rect x="20" y="14" width="80" height="72" rx="10" stroke="var(--pn-text-faint)" stroke-width="3" />
                <line x1="34" y1="38" x2="86" y2="38" stroke="var(--pn-text-faint)" stroke-width="3" stroke-linecap="round" />
                <line x1="34" y1="54" x2="74" y2="54" stroke="var(--pn-text-faint)" stroke-width="3" stroke-linecap="round" />
                <line x1="34" y1="70" x2="60" y2="70" stroke="var(--pn-text-faint)" stroke-width="3" stroke-linecap="round" />
              </svg>
            </template>
          </el-empty>
        </div>
        <div class="grammar" ref="listScrollRef" @scroll="onListScroll" v-show="!show_data" v-loading="loading" element-loading-text="加载中" element-loading-background="var(--pn-loading-bg)">
          <div v-for="item in filteredPhraseList" :key="item.id" class="knowledge">
            <h1>
              {{ item.keyword }}
              <div class="knowledge-actions">
                <el-icon @click="deletePhrase(item)"><DeleteFilled /></el-icon>
                <el-icon @click="edit_current_item(item)"><Edit /></el-icon>
              </div>
            </h1>
            <p v-for="(phrase, i) in item.list" :key="i">
              <span class="phrase-text">{{ phrase.phrase }}</span>
              <span class="translate-text">{{ phrase.translate }}</span>
            </p>
          </div>
          <div v-if="loadingMore" class="load-tip">加载中…</div>
          <div v-else-if="!hasMore && allPhraseList.length > 0" class="load-tip">没有更多了</div>
        </div>
      </div>
      <!-- 添加/编辑词条表单面板 -->
      <transition name="pn-overlay" @before-enter="onPanelBeforeEnter">
      <div class="add-content-panel" v-show="add_content_panel_show" @click.self="closeAddPanel">
        <div class="panel">
          <!-- 抽屉顶部抓手：手机端底部抽屉的可拖拽手柄（点按收起 / 下拉超过阈值关闭、不足回弹）；
               桌面为居中弹窗，grabber 隐藏，不启用拖拽 -->
          <div
            class="panel-grabber"
            @pointerdown="onGrabberDown"
            @pointermove="onGrabberMove"
            @pointerup="onGrabberUp"
            @pointercancel="onGrabberUp"
          ></div>
          <button type="button" id="close-panel" @click="closeAddPanel">×</button>
          <!-- label 置顶：手机端零 hack、桌面更现代；label-width 在 top 模式下被 EP 忽略故移除；
               @submit.prevent 防止回车隐式提交导致 WebView 重载 -->
          <el-form :model="form" label-position="top" @submit.prevent>
            <!-- 类型 + 关键词：基态竖排，≥768px 横排 -->
            <div class="form-row">
              <el-form-item label="类型：" class="form-item-type">
                <el-select v-model="form.type" popper-class="add-select-popper">
                  <el-option label="A~Z" value="A~Z" />
                  <el-option label="#" value="#" />
                </el-select>
              </el-form-item>
              <el-form-item label="关键词:" class="keyword-box">
                <el-input ref="keywordInput" v-model="form.keyword" class="keyword-input" />
                <button type="button" class="keyword-clear" @click="clear_keyword_content">
                  <el-icon><CircleCloseFilled /></el-icon>
                </button>
              </el-form-item>
            </div>
            <el-form-item class="phrase-form-item">
              <el-card style="width: 100%" shadow="never">
                <template #header>
                  <div class="card-header">短语：</div>
                  <button type="button" class="all-delete" @click="allDelete">全部清空</button>
                  <button type="button" class="card-add-btn" @click="appendItem">
                    <el-icon><CirclePlus /></el-icon>
                  </button>
                </template>
                <div class="demo-collapse" ref="formScroll">
                  <el-collapse v-model="activeNames" @change="handleChange" expand-icon-position="left">
                  <transition-group name="flip" tag="div" class="phrase-list">
                  <div v-for="(item, index) in form.list" :key="item.uid" :data-uid="item.uid" class="phrase-content-box" :class="{ 'no-anim': noAnimUids.includes(item.uid), 'dragging': draggingUid === item.uid }" @pointerdown="onDragStart(index, $event)">
                    <div class="phrase-actions">
                      <button type="button" class="phrase-drag-btn" title="拖动排序">
                        <svg viewBox="0 0 1024 1024" aria-hidden="true"><circle cx="320" cy="288" r="56"/><circle cx="704" cy="288" r="56"/><circle cx="320" cy="512" r="56"/><circle cx="704" cy="512" r="56"/><circle cx="320" cy="736" r="56"/><circle cx="704" cy="736" r="56"/></svg>
                      </button>
                      <button type="button" class="phrase-act-btn" title="上移" :disabled="index === 0" @click="moveUp(index)">
                        <svg viewBox="0 0 1024 1024" aria-hidden="true"><path d="M512 160 160 544h192v320h320V544h192z"/></svg>
                      </button>
                      <button type="button" class="phrase-act-btn" title="下移" :disabled="index === form.list.length - 1" @click="moveDown(index)">
                        <svg viewBox="0 0 1024 1024" aria-hidden="true"><path d="M512 864 160 480h192V160h320v320h192z"/></svg>
                      </button>
                      <button type="button" class="phrase-act-btn phrase-insert" title="向上插入" @click="insertAbove(index)">
                        <svg viewBox="0 0 1024 1024" aria-hidden="true"><path d="M512 352 256 608h160v288h192V608h160z"/><rect x="160" y="160" width="704" height="64" rx="24"/></svg>
                      </button>
                      <button type="button" class="phrase-del-btn" title="删除" @click="removeItem(index)">
                        <el-icon><Delete /></el-icon>
                      </button>
                    </div>
                    <el-collapse-item :title="'#' + (index + 1)" :name="String(item.uid)">
                      <el-input type="textarea" :autosize="{ minRows: 1, maxRows: 2 }" v-model="item.phrase" placeholder="请输入短语" />
                      <el-input type="textarea" :autosize="{ minRows: 1, maxRows: 2 }" v-model="item.translate" placeholder="请输入译文" />
                    </el-collapse-item>
                  </div>
                  </transition-group>
                  </el-collapse>
                </div>
              </el-card>
            </el-form-item>
            <div class="form-buttons">
              <el-button type="primary" @click="onSubmit">{{ formMode === 'edit' ? '保存修改' : '添加' }}</el-button>
            </div>
          </el-form>
        </div>
      </div>
      </transition>
    </div>
    <!-- 悬浮右下角加号，打开新增词条面板 -->
    <button id="add-knowledge" @click="openAddForm">+</button>
  </div>
</template>

<script setup lang="ts">
// PhrasePage 的全部逻辑（状态、增删改、分页懒加载、重复追加）抽到独立的组合式函数
import { usePhrasePage } from '../js/PhrasePage.js'
// PhrasePage 公共样式：随本组件懒加载，仅在路由到本页时加载
import '../css/public/PhrasePage.css'

const {
  typeList,
  activeChar,
  handleCharClick,
  show_data,
  search_keyword_input,
  search_keyword_input_content,
  clear_search_input,
  filteredPhraseList,
  deletePhrase,
  edit_current_item,
  add_content_panel_show,
  closeAddPanel,
  form,
  appendItem,
  activeNames,
  handleChange,
  removeItem,
  allDelete,
  onSubmit,
  // 短语条目排序
  moveUp,
  moveDown,
  insertAbove,
  onDragStart,
  noAnimUids,
  draggingUid,
  formMode,
  keywordInput,
  clear_keyword_content,
  openAddForm,
  // 分页懒加载
  allPhraseList,
  listScrollRef,
  formScroll,
  onListScroll,
  listHasMoreBelow,
  loadingMore,
  hasMore,
  loading,
  // 空状态
  isSearching,
  emptyDesc,
  Search,
  // 供 App.vue 导入/导出后刷新词库
  loadDataFromDb,
} = usePhrasePage()

// —— 底部抽屉抓手手势（仅手机端生效，桌面 grabber 已隐藏）——
// 交互：点按 = 收起；按住下拉超过阈值 = 关闭；没拉够 = 回弹。
let dragState: { active: boolean; startY: number; moved: number; pointerId: number } = {
  active: false,
  startY: 0,
  moved: 0,
  pointerId: -1,
}

function panelEl(target: EventTarget | null): HTMLElement | null {
  return (target as HTMLElement | null)?.closest?.('.panel') ?? null
}
function overlayEl(target: EventTarget | null): HTMLElement | null {
  return (target as HTMLElement | null)?.closest?.('.add-content-panel') ?? null
}

function onGrabberDown(e: PointerEvent) {
  if (e.pointerType === 'mouse' && e.button !== 0) return
  const panel = panelEl(e.currentTarget)
  if (!panel) return
  // 清除上一次关闭残留的内联样式，避免拦在离场态
  panel.classList.remove('drag-closing')
  panel.style.transition = ''
  panel.style.transform = ''
  dragState.active = true
  dragState.startY = e.clientY
  dragState.moved = 0
  dragState.pointerId = e.pointerId
  panel.classList.add('dragging')
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}

function onGrabberMove(e: PointerEvent) {
  if (!dragState.active) return
  const panel = panelEl(e.currentTarget)
  if (!panel) return
  const dy = e.clientY - dragState.startY
  dragState.moved = dy
  const y = Math.max(0, dy) // 只允许向下拉，不允许上推
  panel.style.transform = `translateY(${y}px)`
  const overlay = overlayEl(e.currentTarget)
  if (overlay) {
    const h = panel.offsetHeight || 1
    const prog = Math.min(1, y / h)
    overlay.style.opacity = String(1 - prog * 0.5) // 下拉越深遮罩越淡
  }
}

function onGrabberUp(e: PointerEvent) {
  if (!dragState.active) return
  dragState.active = false
  const panel = panelEl(e.currentTarget)
  const overlay = overlayEl(e.currentTarget)
  ;(e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId)
  if (panel) panel.classList.remove('dragging') // 恢复 transform 过渡用于回弹/离场
  const y = Math.max(0, dragState.moved)
  const threshold = Math.min(140, (panel?.offsetHeight || 480) * 0.35)
  const isTap = Math.abs(dragState.moved) <= 8
  if (y > threshold || isTap) {
    // 关闭：自定义下滑离场，禁用标准 reverse slide-up（避免从拖拽位置弹回再滑出）
    if (panel) {
      panel.classList.add('drag-closing')
      panel.style.transition = `transform var(--pn-duration-slow) var(--pn-ease-standard)`
      panel.style.transform = `translateY(${panel.offsetHeight}px)`
    }
    if (overlay) overlay.style.opacity = '' // 交给遮罩淡出动画
    closeAddPanel()
  } else {
    // 回弹到原位
    if (panel) panel.style.transform = 'translateY(0px)'
    if (overlay) overlay.style.opacity = '1'
  }
}

// 进入过渡前清理上一次（拖拽/点按关闭）残留的内联样式。
// 否则重新打开时面板会被残留的 translateY(全高) 推到屏幕外，表现为「闪一下只剩遮罩」。
// 注：面板用 v-show 不重建 DOM，故残留会一直挂着，必须在每次打开前清掉。
function onPanelBeforeEnter(el: Element) {
  const root = el as HTMLElement
  const panel = root.querySelector('.panel') as HTMLElement | null
  if (panel) {
    panel.classList.remove('drag-closing', 'dragging')
    panel.style.transition = ''
    panel.style.transform = ''
  }
  root.style.opacity = ''
}

// 向外暴露方法，给 App.vue【导入完成之后刷新词库】
defineExpose({
  loadDataFromDb
})
</script>

<style scoped>
</style>
