import { onBeforeUnmount, ref, shallowRef } from 'vue'
import { ElMessage } from 'element-plus'
import { Editor, Toolbar } from '@wangeditor/editor-for-vue'
import { Boot } from '@wangeditor/editor'
import '@wangeditor/editor/dist/css/style.css'

// ------------------------------------------------------------
// 富文本工具栏里的「搜索」按钮：唤起 App 的全局查找栏。
// exec 运行在 wangEditor 菜单实例内，拿不到 Vue 的 inject，所以用自定义事件通知 App；
// 事件名与 App.vue 的监听保持一致。
// ------------------------------------------------------------
const SEARCH_ICON_SVG =
  '<svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">' +
  '<path d="M420 30a390 390 0 1 1 0 780 390 390 0 1 1 0-780zm0 140a250 250 0 1 0 0 500 250 250 0 1 0 0-500zm180 530 100-100 290 290-100 100z"/>' +
  '</svg>'

class PnSearchMenu {
  constructor() {
    this.title = '搜索'
    this.iconSvg = SEARCH_ICON_SVG
    this.tag = 'button'
  }
  getValue() { return '' }
  isActive() { return false }
  isDisabled() { return false }
  exec() {
    window.dispatchEvent(new CustomEvent('pn-open-find'))
  }
}

Boot.registerMenu({ key: 'pnSearch', factory: () => new PnSearchMenu() })

// ============================================================
// v-marquee：跑马灯自动横向滚动指令（无缝循环版）
// DOM 结构要求：元素内放两份相同文本（.marquee-chunk），指令驱动 scrollLeft
// 滚到第一份结束时瞬间回卷一份宽度，视觉上无缝衔接、持续流动。
// 交互约定：
//   - 自动模式：RAF 匀速驱动 scrollLeft，滚过一份文本宽度即回卷，重复循环。
//   - 打断：手指/鼠标左键按住期间即可拖拽跟手（捕获指针，移出元素边界也不中断），
//     松开或浏览器接管手势后，延迟恢复自动循环。
//   - 点击 vs 滑动：位移超过阈值判为「滑动」——只滚动文本，并吞掉随后的 click
//     （不进入文件 / 不展开目录）；位移不足判为「点击」——click 照常冒泡，进入文件 / 展开目录。
// 用法：<span v-marquee><span class="marquee-chunk">文本</span>
//                            <span class="marquee-chunk" aria-hidden="true">文本</span></span>
// ============================================================

const SPEED = 40           // 自动滚动速度 px/s
const RESUME_DELAY = 1500  // 松手/失焦后延迟多久恢复自动滚动（ms）
const DRAG_THRESHOLD = 5   // 位移超过该像素才判定为“拖动”，否则视为点击

// 单份文本宽度：判断是否需要跑马灯（文本本身是否超出容器）
function getTextWidth(el) {
  const chunk = el.querySelector('.marquee-chunk')
  return chunk ? chunk.offsetWidth : 0
}

// 回卷周期 = 两份 chunk 起点间距（文本宽 + 间隙 margin），无缝循环用
function getCycleWidth(el) {
  const chunks = el.querySelectorAll('.marquee-chunk')
  if (chunks.length < 2) return 0
  return chunks[1].offsetLeft - chunks[0].offsetLeft
}

// 只有当单份文本超宽时才需要跑马灯；两份拼接后 scrollWidth 必然 > clientWidth，
// 所以不能用 scrollWidth 判断，要用单份宽度对比容器宽度
function needsMarquee(el) {
  return getTextWidth(el) > el.clientWidth + 1
}

// 拖动浏览文本时吞掉随后的 click：click 由 pointerdown→pointerup 同目标派生，
// 在 window 捕获阶段拦截一次并自毁，避免滑动结束误触发行点击（进入文件/展开目录）。
// 返回清理函数，供下一次 pointerdown / 组件卸载时取消残留拦截。
function suppressNextClick() {
  const swallow = (e) => {
    e.stopPropagation()
    e.preventDefault()
    cleanup()
  }
  const cleanup = () => {
    window.removeEventListener('click', swallow, true)
    clearTimeout(timer)
  }
  window.addEventListener('click', swallow, true)
  // click 与 pointerup 同任务或紧随其后的任务派发，都先于本宏任务；
  // 用 setTimeout(0) 兜底自毁，不会误吞下一次交互的 click
  const timer = setTimeout(cleanup, 0)
  return cleanup
}

function createMarqueeState(el) {
  const st = {
    running: false,
    paused: false,
    raf: null,
    lastTs: 0,
    resumeTimer: null,
    ro: null,
    dragging: false,
    moved: false,
    startX: 0,
    startScrollLeft: 0,
    cycleWidth: 0,
    cancelSuppressClick: null,
  }

  const stopRaf = () => { if (st.raf != null) { cancelAnimationFrame(st.raf); st.raf = null } }

  // 按需显示第二份文本（副本）：超宽才需要跑马灯，否则只显示一份，避免「名称 名称」重复
  function ensureActive() {
    const need = needsMarquee(el)
    if (need) {
      el.classList.add('marquee-active') // CSS 用它显示副本 .marquee-chunk + .marquee-chunk
      st.cycleWidth = getCycleWidth(el)
    } else {
      el.classList.remove('marquee-active')
      st.cycleWidth = 0
      el.scrollLeft = 0
    }
    return need
  }

  function tick(ts) {
    st.raf = null
    if (st.paused || st.dragging) return
    const cycle = st.cycleWidth
    if (!cycle) { st.running = false; return }
    if (st.lastTs === 0) st.lastTs = ts
    const dt = Math.min(ts - st.lastTs, 64)
    st.lastTs = ts

    // 无缝循环：滚过一个周期就回卷一个周期，视觉上持续流动无跳变
    let next = el.scrollLeft + (SPEED * dt) / 1000
    if (next >= cycle) next %= cycle
    el.scrollLeft = next
    schedule()
  }
  const schedule = () => { st.raf = requestAnimationFrame(tick) }

  function start() {
    if (st.running) return
    if (!ensureActive()) return
    st.running = true
    st.paused = false
    st.lastTs = 0
    el.scrollLeft = 0
    stopRaf()
    schedule()
  }
  function pause() { st.paused = true; stopRaf() }

  function resume() {
    st.paused = false
    st.dragging = false
    st.moved = false
    st.lastTs = 0
    if (ensureActive()) {
      // 归一化到一个周期内再继续流动：内容是双份重复的，取模后视觉位置不变，
      // 避免松手恢复时跳回起点
      const cycle = st.cycleWidth
      if (cycle > 0) el.scrollLeft = ((el.scrollLeft % cycle) + cycle) % cycle
      st.running = true
      stopRaf()
      schedule()
    } else {
      st.running = false
    }
  }

  function onPointerDown(e) {
    if (e.pointerType === 'mouse' && e.button !== 0) return
    if (!needsMarquee(el)) return // 短文本无需跑马灯，保留原生点击/选中
    // 若上一次拖动的 click 拦截还没自毁，先取消，避免误吞本次正常点击
    if (st.cancelSuppressClick) { st.cancelSuppressClick(); st.cancelSuppressClick = null }
    st.dragging = true
    st.moved = false
    st.startX = e.clientX
    st.startScrollLeft = el.scrollLeft
    pause()
    clearTimeout(st.resumeTimer)
    el.style.userSelect = 'none'
    el.style.webkitUserSelect = 'none'
    // 按住期间捕获指针：即使指针移出元素边界（窄文本很容易竖向漂出），
    // move/up 仍会连续送达本元素，拖拽跟手不会中断，松开前始终可控制滚动
    try { el.setPointerCapture(e.pointerId) } catch { /* 不支持则退化为 window 监听 */ }
  }
  // pointermove / pointerup / pointercancel 绑定 window：双重保险，拖出边界仍能跟手
  function onPointerMove(e) {
    if (!st.dragging) return
    const dx = e.clientX - st.startX
    if (Math.abs(dx) >= DRAG_THRESHOLD) st.moved = true
    if (!st.moved) return
    // 跟手滚动；无缝循环：位置对周期取模，拖到底不会卡住，可无限左右拖
    const cycle = st.cycleWidth
    let target = st.startScrollLeft - dx
    if (cycle > 0) target = ((target % cycle) + cycle) % cycle
    el.scrollLeft = target
  }
  function onPointerUp(e) {
    if (!st.dragging) return
    const wasMoved = st.moved
    st.dragging = false
    st.moved = false
    el.style.userSelect = ''
    el.style.webkitUserSelect = ''
    try { el.releasePointerCapture(e.pointerId) } catch { /* ignore */ }
    if (wasMoved) {
      // 点击 vs 滑动判定：位移超阈值 = 滑动 → 只滚动文本，吞掉随后的 click（不进入文件）
      st.cancelSuppressClick = suppressNextClick()
      st.resumeTimer = setTimeout(resume, RESUME_DELAY)
    } else if (st.running) {
      // 位移不足 = 点击 → 不拦截 click，照常进入文件/展开目录；自动滚动继续
      st.paused = false
      st.lastTs = 0
      stopRaf()
      schedule()
    }
  }
  function onWheel(e) {
    if (Math.abs(e.deltaX) < 1) return
    pause()
    clearTimeout(st.resumeTimer)
    st.resumeTimer = setTimeout(resume, RESUME_DELAY)
  }

  el.addEventListener('pointerdown', onPointerDown)
  el.addEventListener('wheel', onWheel, { passive: true })
  window.addEventListener('pointermove', onPointerMove)
  window.addEventListener('pointerup', onPointerUp)
  // 浏览器接管手势（如纵向滚动）时同上收尾，避免卡死在拖拽态
  window.addEventListener('pointercancel', onPointerUp)
  el.style.touchAction = 'pan-y'

  st.ro = new ResizeObserver(() => {
    if (ensureActive()) {
      if (!st.running && !st.paused && !st.dragging) start()
    } else {
      st.running = false
      stopRaf()
    }
  })
  st.ro.observe(el)

  st.start = start
  st.ensureActive = ensureActive
  st.cleanup = () => {
    stopRaf()
    clearTimeout(st.resumeTimer)
    if (st.cancelSuppressClick) { st.cancelSuppressClick(); st.cancelSuppressClick = null }
    st.ro.disconnect()
    el.removeEventListener('pointerdown', onPointerDown)
    el.removeEventListener('wheel', onWheel)
    window.removeEventListener('pointermove', onPointerMove)
    window.removeEventListener('pointerup', onPointerUp)
    window.removeEventListener('pointercancel', onPointerUp)
  }

  return st
}

export const vMarquee = {
  mounted(el) {
    const st = createMarqueeState(el)
    el.__marquee = st
    st.start()
  },
  updated(el) {
    const st = el.__marquee
    if (!st) return
    // 文本变化（重命名/筛选）后重新评估：超宽则（重）启动并显示副本，否则复位停止
    if (st.ensureActive && st.ensureActive()) {
      if (!st.running && !st.paused && !st.dragging) st.start()
    } else {
      st.running = false
    }
  },
  beforeUnmount(el) {
    const st = el.__marquee
    if (st) { st.cleanup(); delete el.__marquee }
  },
}

// NotePage 的逻辑（富文本编辑器 + 搜索框状态）抽到独立的组合式函数。
export function useNotePage() {
  // 编辑器实例，必须用 shallowRef
  const editorRef = shallowRef()

  // 内容 HTML
  const valueHtml = ref('')

  const toolbarConfig = {
    // 去掉工具栏里的「全屏」按钮（编辑器本身已是整屏，无需再放大）
    excludeKeys: ['fullScreen'],
    // 在工具栏末尾追加自定义「搜索」按钮（与菜单栏的全局搜索同一查找栏）
    insertKeys: { index: 100, keys: ['pnSearch'] },
  }
  // 图片/视频上传：项目没有笔记媒体上传接口，wangEditor 默认 server 为空、
  // base64LimitSize=0，选完文件无处可去。改用 FileReader 读成 data URI 内联进笔记，
  // 这样媒体随笔记内容一起保存 / 备份 / 互传，不依赖本机文件路径。
  // 代价是大文件会撑大笔记，因此设了大小上限并给出提示。
  const MAX_IMAGE_SIZE = 10 * 1024 * 1024
  const MAX_VIDEO_SIZE = 50 * 1024 * 1024
  function fileToDataUrl(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(String(reader.result || ''))
      reader.onerror = () => reject(new Error('读取文件失败'))
      reader.readAsDataURL(file)
    })
  }
  const editorConfig = {
    placeholder: '请输入内容...',
    MENU_CONF: {
      uploadImage: {
        async customUpload(file, insertFn) {
          if (file.size > MAX_IMAGE_SIZE) { ElMessage.error('图片不能超过 10MB'); return }
          try {
            const url = await fileToDataUrl(file)
            insertFn(url, file.name, '')
          } catch (e) {
            ElMessage.error('图片读取失败：' + (e?.message || e))
          }
        },
      },
      uploadVideo: {
        async customUpload(file, insertFn) {
          if (file.size > MAX_VIDEO_SIZE) { ElMessage.error('视频不能超过 50MB'); return }
          try {
            const url = await fileToDataUrl(file)
            insertFn(url, '')
          } catch (e) {
            ElMessage.error('视频读取失败：' + (e?.message || e))
          }
        },
      },
    },
  }

  // 组件销毁时，也及时销毁编辑器
  onBeforeUnmount(() => {
    const editor = editorRef.value
    if (editor == null) return
    editor.destroy()
  })

  const handleCreated = (editor) => {
    editorRef.value = editor // 记录 editor 实例，重要！
  }

  return {
    editorRef,
    valueHtml,
    mode: 'default', // 或 'simple'
    toolbarConfig,
    editorConfig,
    handleCreated,
  }
}
