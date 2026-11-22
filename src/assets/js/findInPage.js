/**
 * 应用内「全局搜索」——DOM 文本查找高亮（对齐浏览器 Ctrl+F 的交互）。
 *
 * 为什么不用浏览器原生 Ctrl+F：Tauri/WebView2 没有公开的 API 直接弹出原生查找栏，
 * 也没有程序化查找高亮接口；本项目改为应用内搜索框 + 自绘高亮，达到同样的效果，
 * 且桌面端 / Android 端行为一致。
 *
 * 两种高亮方式：
 *  1) <mark> 包裹文本节点（默认）——兼容所有 WebView，支持复杂样式（外环/粗体）。
 *  2) CSS Custom Highlight API（编辑区专用）——只创建 Range 交给浏览器绘制，
 *     完全不改动 DOM，所以打字时光标/选区不受干扰、也不会被编辑器当成内容变化。
 *     不支持的 WebView 自动回退到方式 1。
 */

const MARK_CLASS = 'pn-find-mark'
const ACTIVE_CLASS = 'pn-find-mark-active'
const HL_NAME = 'pn-find'
const HL_ACTIVE_NAME = 'pn-find-active'
const SKIP_TAGS = new Set(['SCRIPT', 'STYLE', 'INPUT', 'TEXTAREA', 'SELECT'])
// 不参与查找的区域：顶部选项卡、菜单栏与各页面搜索框、页面左侧导航轨/侧栏、全局查找栏。
// 这些属于操作控件而非正文，高亮它们既没意义又会干扰阅读。
const SKIP_SELECTOR = [
  '.option-panel', '.menu-bar', '.pn-find-bar', '.pn-search',
  '.vrules-type', '.note-side', '.side-item',
  '.type', // 常规短语页 A‑Z / * / # 类型筛选栏：字母是控件不是正文
  // 常规短语页的添加/编辑词条面板：用 v-show 常驻 DOM，关着时里面的
  // 关键词/短语/译文仍会被计入搜索。它是输入表单，不是正文内容，一律排除。
  '.add-content-panel',
  // 无障碍辅助副本：跑马灯把同一标题渲染两份，第二份标了 aria-hidden。
  // 它是视觉冗余，不该被计入搜索（否则 3 个 a 会显示 6 个）。
  '[aria-hidden="true"]'
].join(', ')

function supportsHighlight() {
  return typeof CSS !== 'undefined' && !!CSS.highlights &&
    typeof Highlight !== 'undefined' && typeof Range !== 'undefined'
}

export function createFindInPage() {
  let marks = []          // <mark> 模式：所有命中节点，按文档顺序
  let ranges = []         // Highlight 模式：所有命中 Range，按文档顺序
  let mode = 'mark'       // 'mark' | 'highlight'
  let activeIndex = -1
  // 缓存最近一次查找的容器与关键词：短语页是滑动窗口（滚到底追加、裁掉头部），
  // 被裁掉的卡片里的 <mark> 会脱离 DOM。此时必须能用原词原地重查，否则计数在涨、
  // 当前命中却已不在文档里（"找不到最后一个 a"）。
  let lastRoot = null
  let lastQuery = ''

  /** 还原：mark 模式换回普通文本；Highlight 模式注销浏览器高亮，均不触碰正文结构 */
  function clear() {
    if (mode === 'highlight') {
      if (supportsHighlight()) {
        CSS.highlights.delete(HL_NAME)
        CSS.highlights.delete(HL_ACTIVE_NAME)
      }
      ranges = []
    } else {
      const parents = new Set()
      for (let i = marks.length - 1; i >= 0; i--) {
        const mark = marks[i]
        const parent = mark.parentNode
        if (!parent) continue
        const text = document.createTextNode(mark.textContent || '')
        parent.replaceChild(text, mark)
        parents.add(parent)
      }
      parents.forEach((p) => p.normalize && p.normalize())
      marks = []
    }
    activeIndex = -1
  }

  /** 顺序收集容器内可查找的文本节点（跳过 script/style/输入框/已有高亮） */
  function collectTextNodes(root) {
    const nodes = []
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        if (!node.nodeValue || !node.nodeValue.trim()) return NodeFilter.FILTER_REJECT
        const parent = node.parentElement
        if (!parent) return NodeFilter.FILTER_REJECT
        if (parent.closest('script, style, input, textarea, select')) return NodeFilter.FILTER_REJECT
        if (parent.closest(SKIP_SELECTOR)) return NodeFilter.FILTER_REJECT
        if (parent.closest(`.${MARK_CLASS}`)) return NodeFilter.FILTER_REJECT
        return NodeFilter.FILTER_ACCEPT
      }
    })
    let n
    while ((n = walker.nextNode())) nodes.push(n)
    return nodes
  }

  /** 把普通命中/当前命中两个集合交给浏览器绘制（只在 Highlight 模式调用） */
  function applyHighlights() {
    if (!supportsHighlight()) return
    const normal = ranges.filter((_, i) => i !== activeIndex)
    if (normal.length) CSS.highlights.set(HL_NAME, new Highlight(...normal))
    else CSS.highlights.delete(HL_NAME)
    if (activeIndex >= 0 && ranges[activeIndex]) {
      CSS.highlights.set(HL_ACTIVE_NAME, new Highlight(ranges[activeIndex]))
    } else {
      CSS.highlights.delete(HL_ACTIVE_NAME)
    }
  }

  /**
   * 执行查找。root=要搜索的容器（如 .main-board），query=关键词，
   * preferIndex=重算时希望保留的当前命中序号（0-based，会自动 clamp）。
   * useHighlight=true 时优先用 CSS Highlight API（编辑区），不支持则自动回退 <mark>。
   * 返回 { count, current, behavior }，current 为 1-based 当前命中序号（无命中为 0）。
   */
  function find(root, query, preferIndex, silent, useHighlight = false) {
    clear()
    mode = useHighlight && supportsHighlight() ? 'highlight' : 'mark'
    const q = (query || '').trim()
    lastRoot = root || null
    lastQuery = q
    if (!q || !root) return { count: 0, current: 0, behavior: lastNavBehavior }
    const lowerQ = q.toLowerCase()

    // 先收集文本节点，再逐个处理，避免遍历过程中 DOM 变动影响 TreeWalker
    const nodes = collectTextNodes(root)
    for (const node of nodes) {
      const text = node.nodeValue
      const lower = text.toLowerCase()
      let from = 0
      let pos
      if (mode === 'highlight') {
        // Highlight 模式：只建 Range，不碰 DOM
        while ((pos = lower.indexOf(lowerQ, from)) !== -1) {
          const range = document.createRange()
          range.setStart(node, pos)
          range.setEnd(node, pos + q.length)
          ranges.push(range)
          from = pos + q.length
        }
      } else {
        const parent = node.parentNode
        if (!parent) continue
        const fragment = document.createDocumentFragment()
        let last = 0
        while ((pos = lower.indexOf(lowerQ, from)) !== -1) {
          if (pos > last) fragment.appendChild(document.createTextNode(text.slice(last, pos)))
          const mark = document.createElement('mark')
          mark.className = MARK_CLASS
          mark.textContent = text.slice(pos, pos + q.length)
          fragment.appendChild(mark)
          marks.push(mark)
          last = pos + q.length
          from = pos + q.length
        }
        if (last === 0) continue // 该节点无命中，保持原样
        if (last < text.length) fragment.appendChild(document.createTextNode(text.slice(last)))
        parent.replaceChild(fragment, node)
      }
    }

    const count = mode === 'highlight' ? ranges.length : marks.length
    if (count === 0) return { count: 0, current: 0, behavior: lastNavBehavior }
    // 新关键词从第 1 个开始；内容变化重算时沿用之前的相对位置
    const start = typeof preferIndex === 'number' ? Math.min(Math.max(0, preferIndex), count - 1) : 0
    setActive(start, !silent)
    return result()
  }

  /** 只切换高亮、不滚动：用户自己滚动页面时用它同步"当前命中" */
  function setActiveSilent(index) {
    const count = mode === 'highlight' ? ranges.length : marks.length
    if (count === 0) return
    activeIndex = (index + count) % count
    if (mode === 'highlight') {
      applyHighlights()
    } else {
      if (activeIndex >= 0 && marks[activeIndex]) marks[activeIndex].classList.add(ACTIVE_CLASS)
      marks.forEach((m, i) => { if (i !== activeIndex) m.classList.remove(ACTIVE_CLASS) })
    }
  }

  /** 当前命中是否仍在 DOM 中（滑动窗口裁掉头部后旧节点会脱离文档） */
  function isActiveAttached() {
    if (mode === 'highlight') {
      const r = activeRange()
      return !!r && !!r.startContainer && r.startContainer.isConnected
    }
    const m = activeMark()
    return !!m && m.isConnected
  }

  function activeRange() {
    return activeIndex >= 0 ? ranges[activeIndex] : null
  }
  function activeMark() {
    return activeIndex >= 0 ? marks[activeIndex] : null
  }
  /** 命中元素/Range 所在的最近可滚动祖先 */
  function scrollAncestorOf(el) {
    let p = el.parentElement
    while (p) {
      const cs = getComputedStyle(p)
      if (/(auto|scroll|overlay)/.test(cs.overflowY) || /(auto|scroll|overlay)/.test(cs.overflowX)) return p
      p = p.parentElement
    }
    return document.scrollingElement || document.documentElement
  }
  function rangeElement(range) {
    const n = range.startContainer
    return n && n.nodeType === Node.TEXT_NODE ? n.parentElement : n
  }

  /** 当前命中是否在滚动容器可视区内 */
  function isActiveVisible() {
    if (mode === 'highlight') {
      const range = activeRange()
      if (!range || !range.startContainer || !range.startContainer.isConnected) return false
      const box = range.getBoundingClientRect()
      if (!box || (!box.width && !box.height)) return false
      const el = rangeElement(range)
      if (!el) return false
      const cb = scrollAncestorOf(el).getBoundingClientRect()
      return box.bottom > cb.top + 2 && box.top < cb.bottom - 2
    }
    const mark = activeMark()
    if (!mark || !mark.isConnected) return false
    const box = mark.getBoundingClientRect()
    if (!box.width && !box.height) return false
    const cb = scrollAncestorOf(mark).getBoundingClientRect()
    return box.bottom > cb.top + 2 && box.top < cb.bottom - 2
  }

  function scrollActiveIntoView(behavior = 'smooth') {
    if (mode === 'highlight') {
      const range = activeRange()
      if (!range || !range.startContainer || !range.startContainer.isConnected) return
      const box = range.getBoundingClientRect()
      if (!box || (!box.width && !box.height)) return
      const el = rangeElement(range)
      if (!el) return
      const sc = scrollAncestorOf(el)
      const cb = sc.getBoundingClientRect()
      const delta = (box.top + box.bottom) / 2 - (cb.top + cb.height / 2)
      if (Math.abs(delta) > 1) sc.scrollTo({ top: sc.scrollTop + delta, behavior })
      return
    }
    const mark = activeMark()
    if (!mark || !mark.isConnected) return
    mark.scrollIntoView({ behavior, block: 'center', inline: 'nearest' })
  }

  function setActive(index, scroll = true, behavior = 'smooth') {
    setActiveSilent(index)
    if (scroll) scrollActiveIntoView(behavior)
  }

  /**
   * 短语页滑动窗口裁头后，一部分命中已脱离 DOM；此时数组与页面不同步，
   * next() 滚的是一个孤立节点 → 计数在涨但页面纹丝不动（"找不到最后一个 a"）。
   * 用缓存的原词原地重查，并把当前命中锚定到用户视口里仍可见的那一个，
   * 保证「计数 / 高亮 / 滚动位置」三者始终一致。
   */
  function ensureFresh() {
    const detached = mode === 'highlight'
      ? ranges.some(r => !r.startContainer || !r.startContainer.isConnected)
      : marks.some(m => !m.isConnected)
    if (!detached) return false
    if (!lastRoot || !lastQuery) return false
    find(lastRoot, lastQuery, 0, true, mode === 'highlight')
    let anchor = -1
    let best = Infinity
    const count = mode === 'highlight' ? ranges.length : marks.length
    for (let i = 0; i < count; i++) {
      if (!isNthVisible(i)) continue
      const box = nthBox(i)
      const el = mode === 'highlight' ? rangeElement(ranges[i]) : marks[i]
      const cb = scrollAncestorOf(el).getBoundingClientRect()
      const score = Math.abs((box.top + box.bottom) / 2 - (cb.top + Math.min(80, cb.height * 0.2)))
      if (score < best) { best = score; anchor = i }
    }
    if (anchor >= 0) setActiveSilent(anchor)
    return true
  }
  function nthBox(i) {
    return mode === 'highlight' ? ranges[i].getBoundingClientRect() : marks[i].getBoundingClientRect()
  }
  function isNthVisible(i) {
    const box = nthBox(i)
    if (!box || (!box.width && !box.height)) return false
    const el = mode === 'highlight' ? rangeElement(ranges[i]) : marks[i]
    if (!el || !el.isConnected) return false
    const cb = scrollAncestorOf(el).getBoundingClientRect()
    return box.bottom > cb.top + 2 && box.top < cb.bottom - 2
  }

  // 单次跳转保留平滑动画；但连续快速点击时，后一次 smooth 会取消前一次，
  // 每次都从起步重新计时 → 计数走到末尾而视口几乎不动（"找不到最后一个匹配"）。
  // 所以相邻导航间隔 < 200ms 时降级为即时滚动，停下来后动画自然恢复。
  let lastNavAt = 0
  let lastNavBehavior = 'auto'
  function navBehavior() {
    const now = Date.now()
    const rapid = now - lastNavAt < 200
    lastNavAt = now
    lastNavBehavior = rapid ? 'auto' : 'smooth'
    return lastNavBehavior
  }

  function next() {
    ensureFresh()
    if ((mode === 'highlight' ? ranges.length : marks.length) === 0) return result()
    setActive(activeIndex + 1, true, navBehavior())
    return result()
  }

  function prev() {
    ensureFresh()
    if ((mode === 'highlight' ? ranges.length : marks.length) === 0) return result()
    setActive(activeIndex - 1, true, navBehavior())
    return result()
  }

  function result() {
    const count = mode === 'highlight' ? ranges.length : marks.length
    return {
      count,
      current: count ? activeIndex + 1 : 0,
      behavior: lastNavBehavior
    }
  }

  return {
    find, next, prev, clear, result,
    setActiveSilent, isActiveVisible, scrollActiveIntoView,
    isActiveAttached, ensureFresh,
    getMode: () => mode,
    // 供 App 侧"滚动同步当前命中"使用（Highlight 模式没有 <mark> 可查）
    getHitBoxes: () => (mode === 'highlight' ? ranges : marks).map((r, i) => ({
      box: nthBox(i),
      el: mode === 'highlight' ? rangeElement(r) : r
    }))
  }
}
