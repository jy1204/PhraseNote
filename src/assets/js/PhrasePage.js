// ============================================================
// PhrasePage.js —— PhrasePage.vue 的逻辑组合式函数
// 这里集中存放 PhrasePage 的所有状态与业务逻辑，供 .vue 的
// <script setup> 通过 usePhrasePage() 解构使用。
// 包含：词条增删改、重复关键词“追加”对话框、分页懒加载
//   （启动时加载 PAGE_SIZE 条，滚到底再加载下一页；DOM 用滑动窗口裁剪，
//    恒定最多 MAX_RENDER 个卡片，避免滚到底后几千个节点拖慢滚动）。
// ============================================================

// Tauri
import { invoke } from "@tauri-apps/api/core";
import { listen } from '@tauri-apps/api/event'
// 全局共享状态
import { add_content_panel_show, phraseDataVersion, appReady } from "./index.js"
// 安卓返回键关闭栈：添加/修改面板打开时，返回键 = 点「取消」
import { bindBackClosable } from './backStack.js'
// Element Plus（图标已在 main.js 全局注册，这里只需消息框）
import { ElMessage, ElMessageBox } from 'element-plus'
import { Search } from '@element-plus/icons-vue'
// Vue
import { reactive, ref, onMounted, nextTick, computed, toRaw, onUnmounted, watch } from 'vue'

const PAGE_SIZE = 100
// 滑动窗口：DOM 里最多保留的卡片数。往下滚裁掉头部、往上滚裁掉尾部，
// 被裁掉的数据仍由现有 loadPrev / loadMore 按 windowStart / loadedCount 取回。
// 300 条约 6000 个 DOM 节点，滚动时不再出现「几千个卡片整树重排」。
const MAX_RENDER = 300
const TRIM_STEP = 100

export function usePhrasePage() {
  // 每条短语条目分配稳定 uid：拖拽排序 / 展开状态（activeNames 用 uid 跟踪）
  // 都以条目身份为准，避免用数组下标做 key 导致重排后展开态错位、DOM 复用错乱。
  let uidCounter = 0
  function mkItem(p = {}) {
    return { phrase: p.phrase ?? '', translate: p.translate ?? '', uid: ++uidCounter }
  }
  // 落库前剔除 uid（仅前端排序用，不该写进数据库）
  function cleanList(list) {
    return list.map(({ uid, ...rest }) => rest)
  }

  const formMode = ref('add')
  const editingKeyWord = ref('')

  const typeList = ref([
    '*','A','B','C','D','E','F','G','H','I','J','K','L','M',
    'N','O','P','Q','R','S','T','U','V','W','X','Y','Z','#'
  ])
  const activeChar = ref('*')

  const form = reactive({
    type: 'A~Z',
    first_char: '',
    keyword: '',
    list: [mkItem()]
  })
  const activeNames = ref([String(form.list[0].uid)])

  const savedAddState = ref(null)

  // 主列表数据（分页加载，allPhraseList 只持有已加载的部分）
  const allPhraseList = ref([])
  const show_data = ref(true)
  const search_keyword_input = ref(null)
  const search_keyword_input_content = ref('')

  const keywordInput = ref(null)
  const formScroll = ref(null)
  // 列表滚动容器（.grammar）的 ref，用于滚动到底检测
  const listScrollRef = ref(null)

  // 分页状态
  // windowStart：当前窗口在「后端排序」中的绝对起点。普通加载恒为 0；
  //   编辑保存后的居中窗口会落在中间某处，前后仍能继续双向翻页。
  let windowStart = 0
  const loadingPrev = ref(false)
  // 向前翻页取到空结果时置位，避免滚动事件高频反复发同一个请求
  let prevExhausted = false
  // 窗口刷新期间抑制搜索框 / phraseDataVersion 触发的普通重载，
  // 否则它们会用 offset=0 的第一页覆盖刚定位好的窗口。
  let suppressReload = false
  const loadedCount = ref(0)
  const totalCount = ref(0)
  const loadingMore = ref(false)
  const hasMore = computed(() => loadedCount.value < totalCount.value)
  // 窗口起点之前还有多少条（= windowStart）。windowStart 是普通变量不是响应式，
  // 用两个响应式量相减，模板里的「上方还有 N 条」提示条才能自动更新。
  const aboveCount = computed(() =>
    Math.max(0, loadedCount.value - allPhraseList.value.length)
  )
  // 切换类型 / 搜索时的整体加载遮罩（转圈）
  const loading = ref(false)
  // 保存 / 追加后的「刷新遮罩」：盖住列表，等数据替换 + 滚动定位全部完成后才撤掉。
  // 用户第一眼看到的就是已经停在目标位置的列表，中间任何滚动都看不见。
  const refreshMask = ref(false)

  function snapshotAddForm() {
    savedAddState.value = {
      type: form.type,
      keyword: form.keyword,
      first_char: form.first_char,
      list: form.list.map(p => ({ ...p })),
      activeNames: [...activeNames.value],
    }
  }

  async function openAddForm() {
    formMode.value = 'add'
    editingKeyWord.value = ''
    add_content_panel_show.value = true
    if (savedAddState.value) {
      form.type = savedAddState.value.type
      form.keyword = savedAddState.value.keyword
      form.first_char = savedAddState.value.first_char
      form.list = savedAddState.value.list.map(p => ({ ...p }))
      activeNames.value = [...savedAddState.value.activeNames]
    } else {
      form.type = 'A~Z'
      form.keyword = ''
      form.first_char = ''
      form.list = [mkItem()]
      activeNames.value = [String(form.list[0].uid)]
    }
    await nextTick()
    keywordInput.value?.focus()
  }

  function closeAddPanel() {
    if (formMode.value === 'add') {
      snapshotAddForm()
    }
    add_content_panel_show.value = false
  }

  async function openEditForm(item) {
    if (formMode.value === 'add') {
      snapshotAddForm()
    }
    formMode.value = 'edit'
    editingKeyWord.value = item.keyword
    form.type = item.type
    form.keyword = item.keyword
    form.first_char = item.first_char
    form.list = item.list.map(p => mkItem(p))
    // 编辑时把所有短语条目全部展开（用 uid 跟踪，重排后展开态随条目走）
    activeNames.value = form.list.map(p => String(p.uid))
    add_content_panel_show.value = true
    // 编辑打开时把表单滚动容器重置到顶部（让用户从头浏览该词条的所有短语）
    // 仅作用于打开这一刻，不影响添加模式下 appendItem 往底部滚动的行为
    await nextTick()
    if (formScroll.value) formScroll.value.scrollTop = 0
  }
  function edit_current_item(item) {
    openEditForm(item)
  }

  // 把后端一行（list 为 JSON 字符串）解析成本地条目
  function parseEntry(item) {
    let list = []
    try {
      list = JSON.parse(item.list)
    } catch (e) {
      console.error('list JSON解析失败', item, e)
    }
    return {
      id: item.id,
      type: item.type,
      first_char: item.first_char,
      keyword: item.keyword,
      list: list
    }
  }

  // 当前搜索关键词（去空格、转小写）；为空表示“浏览模式”
  function currentSearchKeyword() {
    return search_keyword_input_content.value.trim().toLowerCase()
  }
  const isSearching = computed(() => currentSearchKeyword() !== '')

  // 空状态文案：搜索无结果时提示“未找到相关短语”，浏览无数据时才提示“去添加”
  const emptyDesc = computed(() => {
    if (isSearching.value) return '未找到相关短语'
    return '这里空空如也，快去添加新知识吧'
  })

  // 按当前模式取一页数据：
  //  - 搜索模式 → 后端全局 search_phrases（不受类型限制）
  //  - 浏览模式 → load_phrases_page 带上当前 char，由后端按类型过滤分页
  // 两者都只取 PAGE_SIZE 条，滚到底再取下一页
  async function fetchPage(offset, limit = PAGE_SIZE) {
    const kw = currentSearchKeyword()
    if (kw) {
      return await invoke('search_phrases', { keyword: kw, offset, limit })
    }
    return await invoke('load_phrases_page', { char: activeChar.value, offset, limit })
  }

  // 加载第一页（重置分页）；导入/导出完成后也走这里刷新
  // force=true 时忽略去重，强制刷新（导入/导出数据已变，即使查询条件相同也要重读）
  // preserveScroll=true 表示「原地刷新」（增删改）：保留当前列表与滚动位置，不清空、
  //   不回顶部，数据回来后按 :key=item.id 做 keyed diff 原地替换。
  //   不带 focusKeyword 时静默（不弹遮罩）；带 focusKeyword 时才盖遮罩隐藏定位过程。
  //   ⚠ 旧实现先 allPhraseList=[] 清空 → 容器塌缩、浏览器把 scrollTop 归零 →
  //     异步查库期间先看到空白，回来后列表从顶部刷出来，最后才在 finally 里把
  //     scrollTop 滚回原位置。这就是「先刷新出列表、再滚动到修改位置」的跳动来源。
  // focusKeyword：仅「撞词追加」还在用（编辑保存已改走 loadPhraseWindow 居中窗口）。
  //   带 focusKeyword 时先用 refreshMask 盖住列表，数据替换 / 补分页 / 滚动定位
  //   全部在遮罩下做完，最后才撤遮罩 —— 用户看到的第一帧就是「已经停在目标位置」
  //   的列表，中间任何滚动都看不见（用户 2026-11-07 要求的交互）。
  //   不带 focusKeyword 的原地刷新（添加 / 删除）保持静默：连遮罩都不闪，位置一寸不动。
  //   ⚠ 窗口模式下（windowStart > 0）原地刷新按窗口起点重取，不跳回 offset=0。
  let lastLoadKey = ''
  let loadSeq = 0
  async function loadDataFromDb(force = false, preserveScroll = false, focusKeyword = '') {
    const kw = currentSearchKeyword()
    const key = kw ? `s:${kw}` : `b:${activeChar.value}`
    // 去重：相同查询条件不重复加载（避免切换类型清空搜索框后又触发一次多余加载）
    if (!force && key === lastLoadKey) return
    lastLoadKey = key
    const seq = ++loadSeq
    // 窗口模式下的「原地刷新」（添加 / 删除）不能跳回 offset=0，否则会破坏
    // 用户刚定位好的居中窗口；按窗口起点重取同一段即可保持位置不动。
    const keepWindow = preserveScroll && windowStart > 0
    if (!keepWindow) windowStart = 0
    prevExhausted = false
    const scrollEl = listScrollRef.value
    const savedScrollTop = preserveScroll && scrollEl ? scrollEl.scrollTop : 0
    // 只有需要「滚动定位」的刷新才盖遮罩。遮罩是不透明层，列表仍在下面正常布局、
    // 可写 scrollTop、可量 rect，所以定位计算完全不受影响。
    const masked = preserveScroll && !!focusKeyword
    if (masked) refreshMask.value = true
    // 换查询条件（切类型 / 搜索 / 导入导出）才清空列表并显示加载遮罩
    if (!preserveScroll) {
      loading.value = true
      allPhraseList.value = []
      loadedCount.value = 0
      totalCount.value = 0
    }
    try {
      // 窗口模式下原地刷新一次取满整个窗口，否则只取 100 条会让列表变矮，
      // 下面「补分页直到够高」的 while 循环要连发多次 IPC 才能撑回原滚动位置。
      const res = await fetchPage(
        keepWindow ? windowStart : 0,
        keepWindow ? MAX_RENDER : PAGE_SIZE
      )
      // 若期间又发起了新的加载，丢弃这次过期结果，避免旧数据覆盖新数据
      if (seq !== loadSeq) return
      const rows = res.rows || []
      allPhraseList.value = rows.map(parseEntry)
      loadedCount.value = windowStart + allPhraseList.value.length
      totalCount.value = res.total ?? 0
      show_data.value = allPhraseList.value.length === 0
    } catch (err) {
      console.error("启动加载数据库失败", err)
      ElMessage({
        message: `加载数据失败：${err}`,
        type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause'
      })
    } finally {
      if (preserveScroll && seq === loadSeq) {
        await nextTick()
        // 再等一拍：watch(filteredPhraseList.length) 的 updateListFade 会改 paddingTop，
        // 等它落定后再量位置，滚动目标才准（此时列表仍被遮罩盖着）。
        await nextTick()
        // 防漂移锁定：DOM 替换 / scroll anchoring 可能偷偷改 scrollTop，按回原值。
        // ⚠ 只能在定位之前用；定位之后再调用会把目标位置拉回去。
        const restore = () => {
          if (!scrollEl) return
          const maxTop = Math.max(0, scrollEl.scrollHeight - scrollEl.clientHeight)
          const target = Math.min(savedScrollTop, maxTop)
          if (Math.abs(scrollEl.scrollTop - target) > 1) scrollEl.scrollTop = target
        }
        restore()
        // 若之前已懒加载了更多分页、滚动位置较深，刷新只重取了第一页，
        // 内容高度不足以支撑原 scrollTop，需要继续补齐分页直到够高
        // ⚠ 每轮校验 loadedCount 是否真的增长：loadMore 可能被 loadingMore 守卫挡住或查库失败，
        //   此时高度不变，若无退出条件会死循环；另设页数上限兜底。
        let guard = 0
        while (hasMore.value && scrollEl && scrollEl.scrollHeight < savedScrollTop + scrollEl.clientHeight && guard++ < 20) {
          const before = loadedCount.value
          // 补页期间禁止裁剪：保持 windowStart 不变，restore / revealKeyword 才准
          await loadMore(false)
          await nextTick()
          restore()
          if (loadedCount.value <= before) break
        }
        // 最后一步：滚到目标词条。整个过程在遮罩下完成，用户看不见。
        if (focusKeyword) revealKeyword(scrollEl, focusKeyword)
        // 补页期间禁止裁剪（否则窗口起点会变、定位基准对不上），定位完成后
        // 统一把 DOM 收敛回 MAX_RENDER。trimHead 自带 rect 补偿，收敛时画面不动。
        let trimGuard = 0
        while (scrollEl && allPhraseList.value.length > MAX_RENDER && trimGuard++ < 20) {
          await trimHead(scrollEl)
        }
      }
      // 遮罩必须等上面全部动作（含滚动定位）做完才撤，否则会露出中间态。
      // 撤掉后 Vue 在微任务里更新 DOM，浏览器绘制时列表已经在目标位置。
      // 故意不加 seq 判断：异常 / 请求被抢占时也必须撤掉，避免遮罩卡死。
      if (masked) refreshMask.value = false
      // 仅当本次仍是“最新一次加载”时才收起转圈，避免被更早的请求误关
      if (seq === loadSeq) loading.value = false
    }
  }

  // 把指定关键词的词条滚到视口内（只在刷新遮罩下调用，用户看不到滚动过程）。
  // ⚠ 用 rect 差值算内容坐标：.grammar 是 grid 容器且未设 position，offsetTop 的
  //   参照对象不是它，直接用 offsetTop 会偏；rect 差值 + scrollTop - paddingTop 才准确。
  // ⚠ 判断条件是「词条顶部是否已在视口内」，不是「整条是否完整可见」：
  //   长词条（height > view）用后者会恒不成立，导致每次保存都滚一下（历史 bug）。
  function revealKeyword(wrap, keyword) {
    if (!wrap) return
    const idx = allPhraseList.value.findIndex(x => x.keyword === keyword)
    if (idx < 0) return   // 不在已加载的页内（如改关键词后跨到很靠后）→ 保持原位置
    const el = wrap.querySelectorAll('.knowledge')[idx]
    if (!el) return
    const pad = parseFloat(getComputedStyle(wrap).paddingTop) || 0
    const top = el.getBoundingClientRect().top - wrap.getBoundingClientRect().top + wrap.scrollTop - pad
    const height = el.offsetHeight
    const view = wrap.clientHeight
    const cur = wrap.scrollTop
    // 顶部已可见 → 完全不动（改了内容但没改关键词时就是这种情况）
    if (top >= cur && top < cur + view) return
    // 短词条居中；长词条顶部对齐，避免把开头切掉
    const target = height <= view ? top - (view - height) / 2 : top
    const max = Math.max(0, wrap.scrollHeight - wrap.clientHeight)
    wrap.scrollTop = Math.max(0, Math.min(target, max))
  }

  // 编辑保存后：以「修改后的关键词」为锚点重新加载一个居中窗口（上下合计 PAGE_SIZE 条）。
  // 不再从 offset=0 重查后逐页补齐，也不做事后滚回 —— 窗口本身就把目标词条放在中间，
  // 全部动作（切类型 / 查库 / 渲染 / 定位）都在不透明遮罩下完成，撤遮罩即最终画面。
  // 关键词没变时也走这里（用户 2026-11-07 定稿：统一重新加载居中窗口）。
  function charForKeyword(kw, current) {
    // 当前是「* 全局」时保持全局，不因为改了关键词就跳进某个字母分类
    if (current === '*') return '*'
    const c = (kw || '').trim().charAt(0)
    if (!c) return '*'
    if (/^[a-z]$/i.test(c)) return c.toUpperCase()
    if (c >= '\u4e00' && c <= '\u9fff') return '#'
    return '*'   // 数字 / 符号开头：退回全局，保证刚改的词条一定可见
  }

  // 把窗口内第 idx 条瞬时滚到视口中央（长词条则顶部对齐）。
  // 用 rect 差值算内容坐标：.grammar 是 grid 且未设 position，offsetTop 参照对象不对。
  function centerWindowItem(wrap, idx) {
    if (!wrap || idx < 0) return
    const el = wrap.querySelectorAll('.knowledge')[idx]
    if (!el) return
    const pad = parseFloat(getComputedStyle(wrap).paddingTop) || 0
    const top = el.getBoundingClientRect().top - wrap.getBoundingClientRect().top + wrap.scrollTop - pad
    const height = el.offsetHeight
    const view = wrap.clientHeight
    const target = height <= view ? top - (view - height) / 2 : top
    const max = Math.max(0, wrap.scrollHeight - wrap.clientHeight)
    wrap.scrollTop = Math.max(0, Math.min(target, max))
  }

  async function loadPhraseWindow(keyword) {
    const wrap = listScrollRef.value
    const targetChar = charForKeyword(keyword, activeChar.value)
    // 先抑制：清空搜索框 / 切类型会触发 watch 里 offset=0 的普通重载，覆盖窗口结果
    suppressReload = true
    refreshMask.value = true
    prevExhausted = false
    try {
      // 搜索模式下改词后新词可能不再匹配搜索词 → 清空搜索并跳到新词条
      if (currentSearchKeyword()) search_keyword_input_content.value = ''
      if (activeChar.value !== targetChar) activeChar.value = targetChar
      await nextTick()   // 让上述响应式变更与被抑制的 watch 先落定

      const res = await invoke('load_phrase_window', {
        keyword,
        char: targetChar,
        limit: PAGE_SIZE,
      })
      const rows = res.rows || []
      allPhraseList.value = rows.map(parseEntry)
      windowStart = res.startOffset ?? 0
      // loadedCount 记「绝对偏移」，这样现有 loadMore 的 fetchPage(loadedCount) 自动对齐窗口末尾
      loadedCount.value = windowStart + allPhraseList.value.length
      totalCount.value = res.total ?? 0
      show_data.value = allPhraseList.value.length === 0
      lastLoadKey = ''   // 窗口之后允许同类筛选再次正常重载

      await nextTick()
      await nextTick()   // 等 updateListFade 的 paddingTop 稳定再定位
      centerWindowItem(wrap, Math.max(0, (res.rank ?? 0) - windowStart))
    } catch (err) {
      console.error('窗口加载失败', err)
      ElMessage({
        message: `刷新失败：${err}`, type: 'error', offset: 50,
        duration: 3000, customClass: 'no-hover-pause'
      })
      // 兜底：退回普通重载，避免停在旧数据上
      loadDataFromDb(true)
    } finally {
      suppressReload = false
      // 不加 seq 判断：异常 / 被抢占时也必须撤掉，避免遮罩卡死
      refreshMask.value = false
      loading.value = false
    }
  }

  // 窗口往前翻一页（用户滚到窗口顶部且 windowStart > 0 时触发）。
  // prepend 会让已有内容整体下移，必须把 scrollTop 补偿回来，否则画面会跳。
  async function loadPrev() {
    if (loadingPrev.value || windowStart <= 0 || prevExhausted) return
    const wrap = listScrollRef.value
    const firstEl = wrap ? wrap.querySelector('.knowledge') : null
    const beforeTop = firstEl ? firstEl.getBoundingClientRect().top : 0
    const take = Math.min(PAGE_SIZE, windowStart)
    loadingPrev.value = true
    try {
      const res = await fetchPage(windowStart - take)
      const rows = res.rows || []
      if (!rows.length) { prevExhausted = true; return }
      allPhraseList.value.unshift(...rows.map(parseEntry))
      windowStart -= rows.length
      loadedCount.value = windowStart + allPhraseList.value.length
      totalCount.value = res.total ?? totalCount.value
      await nextTick()
      if (wrap && firstEl && firstEl.isConnected) {
        // 同一个 keyed 节点被往下推了多少，就多滚多少，视觉上内容原地不动。
        // isConnected 兜底：节点若被 Vue 重建，rect 会是 0，补偿值会把页面拉回顶部。
        wrap.scrollTop += firstEl.getBoundingClientRect().top - beforeTop
      }
      // 往前取回一页后 DOM 超上限：此时视口在开头，裁掉尾部最远的卡片即可，
      // 它们全在视口下方，移除不影响当前画面，也不动 scrollTop。
      trimTail()
    } catch (err) {
      console.error('向上加载失败', err)
      ElMessage({
        message: `加载失败：${err}`, type: 'error', offset: 50,
        duration: 3000, customClass: 'no-hover-pause'
      })
    } finally {
      loadingPrev.value = false
    }
  }

  // ---------- 滑动窗口裁剪 ----------
  // 只裁「远离视口」的一端：loadMore 由滚到底触发，视口一定在末尾 → 裁头部安全；
  // loadPrev 由滚到顶触发，视口一定在开头 → 裁尾部安全。两者都不会裁掉正在看的内容。
  // 裁头部复用 loadPrev 的同一套 rect 补偿：记录裁剪后会成为第 0 条的那个 DOM 节点，
  // DOM 更新后把它的视觉位移补回 scrollTop —— 顶部提示条的出现/消失也自动算进去，
  // 不需要测量卡片高度，也不怕卡片高度不固定。
  async function trimHead(wrap) {
    const cut = Math.min(allPhraseList.value.length - MAX_RENDER, TRIM_STEP)
    if (cut <= 0) return
    const nodes = wrap ? wrap.querySelectorAll('.knowledge') : []
    const anchor = nodes[cut] || null
    const beforeTop = anchor ? anchor.getBoundingClientRect().top : 0
    allPhraseList.value.splice(0, cut)
    windowStart += cut          // loadedCount = windowStart + len 保持不变
    await nextTick()
    if (wrap && anchor && anchor.isConnected) {
      wrap.scrollTop += anchor.getBoundingClientRect().top - beforeTop
    }
  }

  function trimTail() {
    const cut = Math.min(allPhraseList.value.length - MAX_RENDER, TRIM_STEP)
    if (cut <= 0) return
    allPhraseList.value.splice(allPhraseList.value.length - cut, cut)
    // 尾部被裁掉后重新算作「未加载」，之后往下滚会由 loadMore 取回
    loadedCount.value = windowStart + allPhraseList.value.length
  }

  // 滚到底：加载下一页并追加到列表
  // allowTrim=false 给「原地刷新补分页」用：补页期间不能裁头部，否则窗口起点会变，
  // restore() / revealKeyword() 的定位基准就对不上了。
  async function loadMore(allowTrim = true) {
    if (loadingMore.value) return
    if (loadedCount.value >= totalCount.value) return
    loadingMore.value = true
    try {
      const res = await fetchPage(loadedCount.value)
      const rows = res.rows || []
      const parsed = rows.map(parseEntry)
      allPhraseList.value.push(...parsed)
      // 绝对偏移：窗口模式下 windowStart 可能 > 0，不能直接用数组长度
      loadedCount.value = windowStart + allPhraseList.value.length
      totalCount.value = res.total ?? totalCount.value
      // 追加后 DOM 可能超过上限：滚到底时视口在末尾，裁掉头部最远的那些卡片。
      // 先等新卡片进 DOM，trimHead 才量得到将成为第 0 条的锚点节点。
      if (allowTrim) {
        await nextTick()
        await trimHead(listScrollRef.value)
      }
    } catch (err) {
      console.error('加载更多失败', err)
      ElMessage({ message: `加载失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
    } finally {
      loadingMore.value = false
    }
  }

  // 底部渐隐遮罩状态（纯视觉）：列表还没滚到底时置 true。
  // 滚动条已全局隐藏，用伪元素渲染的渐变提示“下面还有内容”；滚到底自动淡出。
  const listHasMoreBelow = ref(false)
  // 顶部留白随滚动实时互补：scrollTop=0 → margin0/padding12；>=12 → margin12/padding0；中间线性互补
  function updateListFade() {
    const el = listScrollRef.value
    if (!el) return
    listHasMoreBelow.value = el.scrollTop + el.clientHeight < el.scrollHeight - 8
    const top = Math.max(0, Math.min(el.scrollTop, 12))
    const mt = top + 'px'
    const pt = (12 - top) + 'px'
    // 值没变就不写：节点多时每帧无条件写样式会反复触发重排
    if (el.style.marginTop !== mt) el.style.marginTop = mt
    if (el.style.paddingTop !== pt) el.style.paddingTop = pt
  }

  let scrollRaf = 0
  function onListScroll() {
    // 一帧最多处理一次：避免高频 scroll 事件里反复读 scrollHeight、写 paddingTop
    // 造成 layout thrashing（DOM 节点多时这是掉帧主因）。
    if (scrollRaf) return
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = 0
      const el = listScrollRef.value
      if (!el) return
      updateListFade()
      // 居中窗口前方还有数据、且滚到窗口顶部 → 往前补一页（双向继续加载）
      if (el.scrollTop <= 24 && windowStart > 0) loadPrev()
      // 距底部 80px 内即触发加载下一页
      if (el.scrollTop + el.clientHeight >= el.scrollHeight - 80) {
        loadMore()
      }
    })
  }

  function clear_search_input() {
    search_keyword_input_content.value = ''
    search_keyword_input.value?.focus()
  }
  function clear_keyword_content() {
    form.keyword = ''
    keywordInput.value?.focus()
  }

  const onSubmit = async () => {
    form.keyword = form.keyword.trim().toLowerCase()
    form.first_char = form.keyword[0] ?? ''
    form.list.forEach(item => {
      item.phrase = item.phrase.trim()
      item.translate = item.translate.trim()
    })

    if (!form.keyword) {
      ElMessage({ message: '关键词不能为空', type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
      return
    }
    if (form.type === '#') {
      if (!/^[\u4e00-\u9fa5]$/.test(form.first_char)) {
        ElMessage({ message: '类型为 # 时，关键词首字符必须是中文', type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
        return
      }
    } else {
      if (!/^[a-zA-Z]$/.test(form.first_char)) {
        ElMessage({ message: '类型为 A~Z 时，关键词首字符必须是字母', type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
        return
      }
    }
    if (form.list.length === 0 || form.list.some(item => !item.phrase)) {
      ElMessage({ message: '短语内容不能为空', type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
      return
    }

    // 重复关键词检查：因列表是分页加载的，用后端 keyword_exists 判断
    // （编辑模式改了关键词才判重，原关键词本身不算重复）
    const exists = await invoke('keyword_exists', { keyword: form.keyword })
    if (exists && form.keyword !== editingKeyWord.value) {
      try {
        await ElMessageBox.confirm(
          `关键词「${form.keyword}」已存在，是否将当前内容追加到该词条？`,
          '提示',
          { confirmButtonText: '追加', cancelButtonText: '取消', type: 'warning' }
        )
      } catch (e) {
        // 取消：什么都不干，停留在表单
        return
      }
      await appendToExisting()
      return
    }

    try {
      const raw = toRaw(form)
      const isEdit = formMode.value === 'edit'
      const oldKeyword = editingKeyWord.value   // 捕获原关键词，避免下面复位后丢失

      // 关键：在「乐观复位表单」之前，先把要写入后端的数据定稿成普通对象（list 提前 stringify 成字符串）。
      // 否则面板复位会改写 form.keyword / form.list（raw 与 form 共享底层对象），
      // 等异步 save 真正读取 raw 时拿到的是被清空后的空表单 —— 数据被覆盖丢失。
      const savePayload = {
        type: raw.type,
        first_char: raw.first_char,
        keyword: raw.keyword,
        list: JSON.stringify(cleanList(raw.list)),
      }

      // 1) 乐观更新：画面瞬时切换，不等后端写入。
      if (isEdit) {
        // 编辑保存完成：关闭面板回到列表。
        // 仅把“原关键词”引用更新为已保存的关键词（便于下次再编辑时正确定位），表单内容不强制清空（面板已关，自然隐藏）。
        editingKeyWord.value = savePayload.keyword
        add_content_panel_show.value = false
      } else {
        // 添加保存完成：保留面板，复位为空白表单并聚焦关键词输入框，便于连续添加
        formMode.value = 'add'
        editingKeyWord.value = ''
        form.keyword = ''
        form.first_char = ''
        form.list = [mkItem()]
        activeNames.value = [String(form.list[0].uid)]
        savedAddState.value = {
          type: form.type,
          keyword: '',
          first_char: '',
          list: [mkItem()],
          activeNames: [String(form.list[0].uid)],
        }
        nextTick(() => keywordInput.value?.focus())
      }

      // 2) 后端异步保存：编辑时先删原行再存（保持原 delete→save 顺序），
      //    完成才弹成功并刷新列表，全程不阻塞 UI（面板已先关）
      ;(async () => {
        if (isEdit) {
          await invoke('delete_phrase', { keyword: oldKeyword })
        }
        await invoke('save_phrase', savePayload)
      })()
        .then(() => {
          ElMessage({
            message: isEdit ? '修改成功' : '保存成功',
            type: 'success', offset: 50, duration: 3000, customClass: 'no-hover-pause'
          })
          // 编辑：以修改后的关键词为中心重新加载 100 条窗口（自动切类型 + 定位），
          //   全程在遮罩下完成，用户第一眼就是已停在目标位置的列表。
          // 添加：面板仍开着要连续录入，走静默原地刷新，不闪遮罩、不定位。
          if (isEdit) loadPhraseWindow(savePayload.keyword)
          else loadDataFromDb(true, true)
        })
        .catch((err) => {
          console.error('保存失败', err)
          ElMessage({ message: `保存失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
        })
    } catch (err) {
      console.error('保存前置失败', err)
      ElMessage({ message: `保存失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
    }
  }

  // 关键词重复时，把当前表单内容追加到已有词条（后端按 phrase 去重）
  // 乐观更新：先关面板+复位表单（瞬时切画面），再后台跑 delete_phrase→append_phrase，
  // 跑完才弹“已追加到已有词条”并刷新列表，全程不阻塞 UI（面板已先关）。
  function appendToExisting() {
    const raw = toRaw(form)
    const targetKeyword = form.keyword
    const listStr = JSON.stringify(cleanList(raw.list))
    // 编辑模式下改了关键词触发重复：记下面板打开时的原词条关键词，用于先删原行
    const editingKeyword = editingKeyWord.value

    // 1) 乐观更新：追加完成保留面板（便于继续录入），仅复位表单内容，不等后端写入
    formMode.value = 'add'
    editingKeyWord.value = ''
    form.keyword = ''
    form.first_char = ''
    form.list = [mkItem()]
    activeNames.value = [String(form.list[0].uid)]
    savedAddState.value = {
      type: form.type,
      keyword: '',
      first_char: '',
      list: [mkItem()],
      activeNames: [String(form.list[0].uid)],
    }
    nextTick(() => keywordInput.value?.focus())

    // 2) 后端异步追加：完成才弹成功并刷新列表，不阻塞 UI（面板已先关）
    ;(async () => {
      if (editingKeyword && editingKeyword !== targetKeyword) {
        await invoke('delete_phrase', { keyword: editingKeyword })
      }
      await invoke('append_phrase', {
        keyword: targetKeyword,
        list: listStr,
      })
    })()
      .then(() => {
        ElMessage({ message: '已追加到已有词条', type: 'success', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
        // 盖遮罩隐藏刷新 + 定位过程，完成后显示已停在目标词条的列表
        loadDataFromDb(true, true, targetKeyword)
      })
      .catch((err) => {
        console.error('追加失败', err)
        ElMessage({ message: `追加失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
      })
  }

  async function deletePhrase(item) {
    try {
      await ElMessageBox.confirm(`确定删除「${item.keyword}」吗？`, '提示', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      })
      await invoke('delete_phrase', { keyword: item.keyword })
      ElMessage({ message: '删除成功', type: 'success', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
      await loadDataFromDb(true, true)
    } catch (err) {
      if (err === 'cancel') return
      console.error('删除失败', err)
      ElMessage({ message: `删除失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
    }
  }

  async function removeItem(index) {
    try {
      await ElMessageBox.confirm('确定删除这一条短语吗？', '提示', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      })
    } catch (e) {
      return
    }
    form.list.splice(index, 1)
    // 删除后保持剩余条目全部展开（用 uid 跟踪）
    activeNames.value = form.list.map(p => String(p.uid))
  }
  async function allDelete() {
    try {
      await ElMessageBox.confirm('确定清空所有短语吗？', '提示', {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      })
    } catch (e) {
      return
    }
    form.list = []
    activeNames.value = []
    appendItem()
  }
  async function appendItem() {
    const fresh = mkItem()
    form.list.push(fresh)
    activeNames.value.push(String(fresh.uid))
    // 新追加的空条目同样瞬间展开（关掉折叠动画），保证滚到底部定位精确、无偏差
    noAnimUids.value.push(fresh.uid)
    setTimeout(() => {
      noAnimUids.value = noAnimUids.value.filter(u => u !== fresh.uid)
    }, 360)
    // 等高度落定后再定位（autosize 会在 setTimeout 里改 textarea 高度），
    // 直接 nextTick 定位会算到尚未稳定的尺寸 → 滚动位置偏差
    await settleThenScroll(fresh.uid, 'end')
  }
  // 平滑滚动到指定 scrollTop（只动列表自身的滚动容器，不牵动外层容器）
  // CSS 未给容器设 scroll-behavior，故直接赋值 scrollTop 永远是即时的；
  // 这里显式用 JS 的 behavior:'smooth' 做平滑滚动（不支持时降级为瞬时滚动）。
  function scrollWrapTo(wrap, top) {
    const max = Math.max(0, wrap.scrollHeight - wrap.clientHeight)
    const target = Math.max(0, Math.min(top, max))
    if (Math.abs(wrap.scrollTop - target) < 1) return
    let smooth = false
    try {
      smooth = !!(window.CSS && CSS.supports && CSS.supports('scroll-behavior', 'smooth'))
    } catch (e) { smooth = false }
    if (smooth && typeof wrap.scrollTo === 'function') {
      try {
        wrap.scrollTo({ top: target, behavior: 'smooth' })
        return
      } catch (e) { /* 降级为瞬时滚动 */ }
    }
    wrap.scrollTop = target
  }
  // 精确定位某个条目：
  // ⚠ 用 offsetTop/offsetHeight（布局坐标）而不是 getBoundingClientRect：
  //   列表重排时 Vue 的 transition-group 会给条目加 FLIP transform（动画中间态），
  //   此时 rect 量到的是“动画开始前的旧位置” → 滚动到错误位置（表现为没滚到目标/有偏差）。
  //   offsetTop 不受 transform 影响，恒等于最终落位的位置。
  function scrollPhraseIntoView(uid, align = 'center') {
    const wrap = formScroll.value
    if (!wrap) return
    const el = wrap.querySelector(`.phrase-content-box[data-uid="${uid}"]`)
    if (!el) return
    const top = el.offsetTop
    const height = el.offsetHeight
    const view = wrap.clientHeight
    let target
    if (align === 'end') target = top + height - view
    else if (align === 'start') target = top
    else target = top - (view - height) / 2
    scrollWrapTo(wrap, target)
  }
  // 等 DOM 与条目高度落定后再定位：
  // EP 的 el-input 用 autosize，在挂载后的 setTimeout + nextTick 里才确定 textarea 高度
  // （插入的是空条目，高度会随后变化）→ 太早定位会按未稳定的高度算目标 → 位置偏差。
  async function settleThenScroll(uid, align = 'center') {
    const wrap = formScroll.value
    if (!wrap) return
    await nextTick()
    // 逐帧等待高度稳定（最多 12 帧）：连续 3 帧高度不变才认为已稳定
    let last = -1
    let stable = 0
    for (let i = 0; i < 12; i++) {
      await new Promise(r => requestAnimationFrame(r))
      const el = wrap.querySelector(`.phrase-content-box[data-uid="${uid}"]`)
      const cur = el ? el.offsetHeight : 0
      if (cur > 0 && cur === last) {
        stable++
        if (stable >= 2) break
      } else {
        stable = 0
      }
      last = cur
    }
    scrollPhraseIntoView(uid, align)
  }
  const handleChange = (val) => {
    console.log(val)
  }

  // —— 短语条目排序：上移 / 下移 / 向上插入 / 拖拽重排 ——
  // 全部基于 form.list 数组顺序，落库时按数组顺序即最终展示顺序。
  function moveTo(from, to) {
    if (from < 0 || to < 0 || from >= form.list.length || to >= form.list.length || from === to) return
    const [moved] = form.list.splice(from, 1)
    form.list.splice(to, 0, moved)
  }
  function moveUp(index) {
    if (index <= 0) return
    moveTo(index, index - 1)
    // 上移后把该条目一次性滚到视口中央（条目已展开，nextTick 即可）
    const uid = form.list[index - 1].uid
    nextTick(() => scrollPhraseIntoView(uid))
  }
  function moveDown(index) {
    if (index >= form.list.length - 1) return
    moveTo(index, index + 1)
    const uid = form.list[index + 1].uid
    nextTick(() => scrollPhraseIntoView(uid))
  }
  // 需要“瞬间展开、不做折叠动画”的条目 uid 集合（插入空条目时用，定位才精确）。
  // 用数组+includes 以便模板响应式判断；动画时长后移除，该条目之后收起/展开恢复正常动画。
  const noAnimUids = ref([])
  function insertAbove(index) {
    const fresh = mkItem()
    form.list.splice(index, 0, fresh)
    activeNames.value.push(String(fresh.uid))
    // 关掉这条新条目的折叠动画 → 它一插入就是展开态、高度稳定，nextTick 居中定位才精确无偏差；
    // 动画时长后再放开（no-anim 移除），不影响之后手动收起/展开。
    noAnimUids.value.push(fresh.uid)
    setTimeout(() => {
      noAnimUids.value = noAnimUids.value.filter(u => u !== fresh.uid)
    }, 360)
    // 等高度落定（textarea autosize 在 setTimeout 里改高度）后再居中定位，
    // 否则会按尚未稳定的高度算目标位置 → 滚动出现偏差
    settleThenScroll(fresh.uid)
  }

  // —— 拖拽自动滚动速度 ——
  // 固定 px/秒：不随窗口高度、列表条目数、指针超出距离变化；列表与外层视口共用一个值。
  // 想调快慢只改这一个常量即可。
  // dt 仅把 px/秒换算成每帧距离（帧率解耦，60Hz / 120Hz 手感一致），不参与速度计算。
  const AUTO_SCROLL_SPEED = 260

  // —— 触摸手势意图判定（仅"触摸 + 按在折叠标题上"时启用）——
  // 标题区设了 touch-action:none，浏览器不再接管滚动，必须由 JS 区分两种意图：
  //   · 按下后立刻快速竖向滑动 → "滚动查看"，按手指位移 1:1 滚列表，不生成克隆体
  //   · 按住一小会儿再移动       → "拖动排序"，走原拖拽链路
  // ⚠ touch-action 在 pointerdown 时就被浏览器锁定，手势开始后再改无效，
  //   所以标题区必须在 CSS 基态就声明 none，不能靠这里动态切换。
  const DRAG_INTENT_MS = 150    // 意图窗口：按住超过此时长再移动 = 拖拽
  const SCROLL_INTENT_PX = 8    // 窗口内竖向位移超过此值 = 滚动

  // 拖拽重排状态（非响应式，仅拖拽过程用）。
  // 采用“原位占位 + 跟随手指的克隆体”方案：被拖条目在列表中实时换位（其它条目同步挪动），
  // 同时一个固定定位的克隆体精确跟随手指，松手即落位 —— 既“跟手”又“实时调整”。
  const drag = {
    active: false, el: null, clone: null, uid: null,
    fromIndex: -1, placeholderIndex: -1,
    startX: 0, startY: 0, startScroll: 0,
    startTime: 0,     // 按下时刻（performance.now），触摸意图窗口计时
    intent: '',       // 手势意图：'' | 'pending' | 'drag' | 'scroll'
    grabDX: 0, grabDY: 0, width: 0,
    started: false,   // 是否已进入拖拽态（克隆体已生成）
    moved: false,     // 是否真的移动过（超过位移阈值）
    grip: false,      // 是否从"拖拽手柄"按下（灵敏度更高，与标题同一套代码）
    pointerY: 0,      // 最近一次指针 Y（视口坐标）——帧循环据此判断滚动方向
    lastFrameY: -1, // 上一帧的指针 Y（用于判断是否需要重算落点）
    lastTs: 0,       // 上一帧的时间戳（用于算 dt，让滚动速度与帧率解耦）
    rafId: 0,         // 拖拽帧循环句柄（rAF 驱动，与指针是否移动无关）
    cardBody: null,   // .el-card__body（超出它 → 滚动外层视口）
    scrollers: [],    // 外层可滚动容器（表单/窗口），按由内到外排列
    pressHeader: null // 按下时命中的折叠标题（拖完用于精确抑制那一次误点击）
  }
  // 当前被拖条目的 uid（响应式），用于模板 :class 绑定 dragging 占位样式，
  // 这样 Vue 实时重排 DOM 时该样式不会被 patch 覆盖掉。
  const draggingUid = ref(null)
  // FLIP 动画是否被抑制（滚动期间为 true）。非响应式：直接改 DOM class，避免触发额外重渲染。
  let flipSuppressed = false
  // 新一次 pointerdown 前的兜底清理。
  // ⚠ 手机上多指 / 快速连拖时，上一轮可能还没收到 pointerup 就又按下：
  //    若不先清，drag.clone 会被直接覆盖成 null，旧克隆体永久留在 DOM 里
  //    → 用户看到"上一个克隆体停在那里不消失"。
  function resetDrag() {
    stopDragLoop()
    if (drag.clone && drag.clone.parentNode) drag.clone.parentNode.removeChild(drag.clone)
    drag.clone = null
    draggingUid.value = null
    setFlipSuppressed(false)
    setDraggingClass(false)
    window.removeEventListener('pointermove', onDragMove)
    window.removeEventListener('pointerup', onDragEnd)
    window.removeEventListener('pointercancel', onDragEnd)
    drag.active = false
  }
  // 鼠标左键 / 手指按下即开始监听；在盒子标题区（含拖拽手柄）按下可拖，
  // 文本域、输入框与各操作按钮（删除/上下移/插入）按下不触发拖拽，保持原生交互。
  // ⚠ 拖拽手柄与标题拖动走的是同一套代码：都在这里登记状态，
  //   之后由 beginDrag/positionClone/moveTo/onDragEnd 一条链路处理，
  //   区别只有"生效阈值"（手柄 0px 立即生效，标题需移动 6px 才生效，避免与点击展开冲突）。
  function onDragStart(index, e) {
    if (e.pointerType === 'mouse' && e.button !== 0) return
    const t = e.target
    if (t.closest('textarea') || t.closest('input')) return
    if (t.closest('.phrase-del-btn') || t.closest('.phrase-act-btn')) return
    const isGrip = !!t.closest('.phrase-drag-btn')
    const header = t.closest('.el-collapse-item__header')
    const isTouch = e.pointerType !== 'mouse'
    // 触摸时只有"拖拽手柄"和"折叠标题"能进入拖拽；折叠内容 / 空白区一律
    // 留给浏览器原生滚动查看（那里 touch-action 仍是 manipulation）。
    if (isTouch && !isGrip && !header) return
    // 兜底：先清掉上一轮可能残留的克隆体 / 帧循环 / 全局样式，再登记本次状态
    resetDrag()
    const el = t.closest('.phrase-content-box')
    if (!el) return
    const wrap = formScroll.value
    const rect = el.getBoundingClientRect()
    drag.active = true
    drag.el = el
    drag.uid = form.list[index]?.uid ?? null
    drag.fromIndex = index
    drag.placeholderIndex = index
    drag.startX = e.clientX
    drag.startY = e.clientY
    drag.startScroll = wrap ? wrap.scrollTop : 0
    drag.startTime = performance.now()
    // 鼠标和手柄沿用"按下即拖"；只有触摸按在标题上才需要意图判定
    drag.intent = (isTouch && !isGrip) ? 'pending' : 'drag'
    drag.grabDX = e.clientX - rect.left
    drag.grabDY = e.clientY - rect.top
    drag.width = rect.width
    drag.moved = false
    drag.started = false
    drag.grip = isGrip
    drag.pointerY = e.clientY
    drag.lastFrameY = -1
    drag.lastTs = 0
    drag.clone = null
    // 记录卡片区与外层可滚动容器：指针拖出 .el-card__body 时滚动外层视口
    drag.cardBody = el.closest('.el-card__body')
    drag.scrollers = collectScrollers(drag.cardBody)
    // 记录按下时命中的折叠标题：拖完后只需抑制“这一处”的误点击
    drag.pressHeader = header
    if (isGrip && e.cancelable) e.preventDefault()   // 手柄：阻止默认行为（选中文字/聚焦）
    window.addEventListener('pointermove', onDragMove, { passive: false })
    window.addEventListener('pointerup', onDragEnd, { passive: false })
    window.addEventListener('pointercancel', onDragEnd, { passive: false })
  }
  // 真正开始拖拽：生成立体克隆体跟随手指，原位条目降级为虚线落位框。
  // 手柄与标题拖动都调用这里（同一套代码），started/moved 在此统一置位。
  function beginDrag() {
    const el = drag.el
    if (!el) return
    const rect = el.getBoundingClientRect()
    const clone = el.cloneNode(true)
    clone.classList.add('dragging-clone')
    clone.classList.remove('dragging')
    // 宽度锁定为原条目的实际尺寸（.dragging-clone 为 border-box，与原盒子盒模型一致）
    clone.style.width = rect.width + 'px'
    // cloneNode 只复制 textarea 的默认值，而 Vue 写入的是 .value 属性 → 手动同步实时文本
    const srcAreas = el.querySelectorAll('textarea')
    const dstAreas = clone.querySelectorAll('textarea')
    for (let i = 0; i < dstAreas.length && i < srcAreas.length; i++) {
      dstAreas[i].value = srcAreas[i].value
    }
    // 关键：克隆体挂在卡片区内（.el-card__body 子树），这样所有
    // .panel>form .el-card__body … 作用域样式（padding、.phrase-actions 绝对定位、
    // 按钮尺寸等）才能命中，外观与原条目完全一致；它仍是 position:fixed（面板静止时无
    // transform 祖先 → 相对视口），用 clientX/clientY 精确跟随手指。
    const host = el.closest('.el-card__body') || document.body
    host.appendChild(clone)
    drag.clone = clone
    draggingUid.value = drag.uid
    drag.started = true
    drag.moved = true
    // 拖拽期间关闭列表的 CSS 滚动锚定，避免浏览器自动改 scrollTop 与手动滚动打架
    setDraggingClass(true)
    positionClone(drag.startX, drag.startY)
    // 启动帧循环：自动滚动与落点更新按指针位置每帧进行（指针停留也会持续滚）
    startDragLoop()
  }
  // 克隆体跟随指针：left/top 固定为 0，用 transform 位移（不触发布局，仅合成器工作），
  // 左上角精确对齐抓取点 → 实时跟手、零延迟。
  function positionClone(clientX, clientY) {
    if (!drag.clone) return
    const x = clientX - drag.grabDX
    const y = clientY - drag.grabDY
    drag.clone.style.transform = `translate3d(${x}px, ${y}px, 0)`
  }
  // 算出被拖条目应该插入到“去掉它自己之后”的列表里的第几位。
  // ⚠ 用布局坐标（offsetTop）而非 getBoundingClientRect：重排时其它条目正在做 FLIP
  //    位移动画，rect 量到的是动画中间态 → 落点判定会漂移。
  // 规则：指针上方（含中线之上）的其它条目数量 = 插入位。
  //    返回值 k 是“移除被拖条目后的数组下标”，正好对应 moveTo(from, k) 的语义。
  function insertIndexFor(clientY) {
    const wrap = formScroll.value
    if (!wrap) return drag.placeholderIndex
    // 指针位置夹回可见范围：越界时不把落点钉到数组末尾（"最新的位置"），
    // 而是落在可见区边缘，随滚动自然推进。
    const contentY = clampContentY(wrap, clientY)
    const boxes = Array.from(wrap.querySelectorAll('.phrase-content-box'))
      .filter(b => String(b.dataset.uid) !== String(drag.uid))
    let k = 0
    for (const b of boxes) {
      const mid = b.offsetTop + b.offsetHeight / 2
      if (contentY < mid) break
      k++
    }
    return k
  }
  // 指针 Y（视口坐标）→ 滚动容器内容坐标（与 offsetTop 同一坐标系）
  function contentYOf(wrap, clientY) {
    const wrapRect = wrap.getBoundingClientRect()
    return clientY - wrapRect.top + wrap.scrollTop
  }
  // 把指针的内容坐标夹回列表的可见范围（越界时取最近的边缘）。
  // ⚠ 为什么需要：指针拖到列表外很远时，contentY 会远超所有条目的中线
  //    → 落点被直接钉到数组末尾（"最新的位置"），要一路滚到底才看得见落点在哪。
  //    夹回可见区后，落点始终在视口内可见，随滚动自然地逐项推进。
  function clampContentY(wrap, clientY) {
    const raw = contentYOf(wrap, clientY)
    const top = wrap.scrollTop
    const bottom = top + wrap.clientHeight
    return Math.max(top, Math.min(bottom, raw))
  }
  // 收集 .el-card__body 之外所有可滚动的祖先容器（overflow-y 为 auto/scroll/overlay），
  // 末尾追加文档滚动元素 —— 指针拖出卡片区时按顺序尝试滚动，能滚动的先滚。
  function collectScrollers(cardBody) {
    const list = []
    let p = cardBody ? cardBody.parentElement : null
    while (p && p !== document.body) {
      const st = getComputedStyle(p)
      if (/(auto|scroll|overlay)/.test(st.overflowY || '')) list.push(p)
      p = p.parentElement
    }
    list.push(document.scrollingElement || document.documentElement)
    return list
  }
  // 指针在列表滚动容器内贴近上下边缘时，滚动列表自身，方便在长列表里拖到远处。
  // 阈值 24px；速度固定 AUTO_SCROLL_SPEED（px/秒，乘 dt 换算成每帧距离）。
  // 返回是否真的滚动了。
  function autoScroll(clientY, dt) {
    const wrap = formScroll.value
    if (!wrap) return false
    const r = wrap.getBoundingClientRect()
    const threshold = 24
    const before = wrap.scrollTop
    if (clientY < r.top + threshold) wrap.scrollTop -= AUTO_SCROLL_SPEED * dt
    else if (clientY > r.bottom - threshold) wrap.scrollTop += AUTO_SCROLL_SPEED * dt
    return wrap.scrollTop !== before
  }
  // 指针超出 .el-card__body 上下边界 → 滚动外层视口，
  // 让长列表两端（或被窗口裁掉的部分）也能被拖到。
  // 速度同样固定 AUTO_SCROLL_SPEED，不按超出距离加速。
  // 返回是否真的滚动了（供调用方决定是否退回复用列表滚动）。
  function autoScrollViewport(clientY, dt) {
    const cardBody = drag.cardBody
    if (!cardBody) return false
    const r = cardBody.getBoundingClientRect()
    let speed = 0
    if (clientY < r.top) speed = -AUTO_SCROLL_SPEED
    else if (clientY > r.bottom) speed = AUTO_SCROLL_SPEED
    if (!speed) return false
    // 逐个尝试外层可滚动容器，第一个能真正滚动的即生效；全滚不动则退到文档滚动元素
    for (const sc of drag.scrollers) {
      if (!sc) continue
      const before = sc.scrollTop
      sc.scrollTop += speed * dt
      if (sc.scrollTop !== before) return true
    }
    return false
  }
  // 拖拽期间的自动滚动（按指针位置分流）：
  // ① 指针超出 .el-card__body → 先滚外层视口（"拖出卡片区自动滚视口"的既定行为）；
  //    外层一旦滚到边界（矮窗口下常只有十几 px 可滚），这一帧立即改滚列表，
  //    避免出现"外层到底 → 整段时间完全不动"的空档。
  // ② 指针在卡片区内、贴近列表上下边缘 → 滚列表自身。
  // ⚠ 不能让"列表永远优先"：矮窗口下卡片区被压到 ~142px，.demo-collapse 是
  //    height:100% 填满 cardBody 的，两者上下边界几乎重合 —— 指针一出界，
  //    同时就落在列表边缘 24px 阈值内，列表会抢走每一帧，外层视口永远轮不到滚，
  //    表现为"拖出卡片区后自动滚动特别慢 / 几乎不动"。
  //    现在改为"出界时视口优先 + 视口到底自动回退列表"，两个轴都能持续推进。
  // 返回是否发生了滚动（滚动会改变指针下的内容位置 → 需重算落点）。
  function autoScrollDrag(clientY, dt) {
    const cardBody = drag.cardBody
    let outside = false
    if (cardBody) {
      const c = cardBody.getBoundingClientRect()
      outside = clientY < c.top || clientY > c.bottom
    }
    if (outside && autoScrollViewport(clientY, dt)) return true
    return autoScroll(clientY, dt)
  }
  // 拖拽帧循环（rAF 驱动）：
  // ⚠ 关键：自动滚动由"指针位置"决定，而不是由 pointermove 事件触发 ——
  //    若靠 pointermove 触发，指针停在卡片区外不动时滚动就会中断。
  //    这里每帧都根据当前指针位置重新判定并滚动，停留/移动行为一致。
  function dragFrame(ts) {
    if (!drag.active) return
    // 用时间差（dt）换算滚动距离 → 滚动速度与帧率解耦（60Hz/120Hz 手感一致）
    const dt = drag.lastTs ? Math.min(0.05, (ts - drag.lastTs) / 1000) : 0
    drag.lastTs = ts
    const clientY = drag.pointerY
    // ① 自动滚动：指针在卡片区内 → 滚列表；超出卡片区 → 滚外层视口
    const scrolled = dt > 0 ? autoScrollDrag(clientY, dt) : false
    // ② 滚动期间给列表加 no-flip：抑制 FLIP 位移动画。
    //    原因：FLIP 用 getBoundingClientRect() 测量，而该测量包含元素当前未归零的
    //    transform —— 滚动中每帧都在重排，上一帧的动画还没结束就重新测量，
    //    基线被污染 → 动画朝错误起点滑 → 反复重启 = 视觉"晃/晕"。
    //    滚动本身已是连续运动，此时让条目直接就位（snap）反而更稳。
    setFlipSuppressed(scrolled)
    // ③ 实时重排：指针移动过、或滚动改变了指针下的内容位置 → 重算落点
    //    （两者都没发生就跳过，避免每帧做无谓的 DOM 查询与布局读取）
    if (scrolled || clientY !== drag.lastFrameY) {
      drag.lastFrameY = clientY
      const k = insertIndexFor(clientY)
      if (k !== drag.placeholderIndex) {
        moveTo(drag.placeholderIndex, k)
        drag.placeholderIndex = k
      }
    }
    drag.rafId = requestAnimationFrame(dragFrame)
  }
  // 开关列表的 FLIP 动画（滚动期间抑制，避免"晕"）。
  // 挂在 <html> 上而不是列表元素上 —— Vue 永远不会 patch <html> 的 class，
  // 所以这个开关绝对不会被渲染流程覆盖掉。
  function setFlipSuppressed(on) {
    if (flipSuppressed === on) return
    flipSuppressed = on
    document.documentElement.classList.toggle('pn-no-flip', on)
  }
  // 拖拽期间关闭列表的 CSS 滚动锚定（overflow-anchor: none）：
  // 浏览器锚定会在重排时自动改 scrollTop，和我们手动控制的滚动互相打架，
  // 表现为滚动忽快忽慢 + 突然跳一下。落点由 insertIndexFor 精确算，不需要锚定兜底。
  function setDraggingClass(on) {
    document.documentElement.classList.toggle('pn-dragging', on)
  }
  function startDragLoop() {
    if (drag.rafId) return
    drag.rafId = requestAnimationFrame(dragFrame)
  }
  function stopDragLoop() {
    if (drag.rafId) {
      cancelAnimationFrame(drag.rafId)
      drag.rafId = 0
    }
  }
  function onDragMove(e) {
    if (!drag.active) return
    const dx = e.clientX - drag.startX
    const dy = e.clientY - drag.startY
    // 记录指针位置：帧循环据此决定滚动方向与落点（与指针是否移动无关）
    drag.pointerY = e.clientY
    // 触摸按在标题上：先用意图窗口区分"滚动查看"与"拖动排序"。
    // 标题区 touch-action:none 已禁掉浏览器原生滚动，判为滚动时由 JS 1:1 跟手代滚。
    if (drag.intent === 'pending') {
      const elapsed = performance.now() - drag.startTime
      const absX = Math.abs(dx), absY = Math.abs(dy)
      if (absY > SCROLL_INTENT_PX && absY >= absX) {
        drag.intent = 'scroll'
      } else if (elapsed >= DRAG_INTENT_MS || (absX > SCROLL_INTENT_PX && absX > absY)) {
        drag.intent = 'drag'
      } else {
        return   // 窗口内还没判出来：不接管、不生成克隆体，也不阻止点击
      }
    }
    if (drag.intent === 'scroll') {
      const wrap = formScroll.value
      if (wrap) wrap.scrollTop = drag.startScroll - dy
      // 手势确实移动过 → 抑制抬起时可能落在标题上的误点击（展开/收起）
      drag.moved = true
      return
    }
    // ⚠ 拖拽手柄与标题拖动走的是同一套判定与处理链路，唯一区别是生效阈值：
    //    · 手柄（.phrase-drag-btn）：阈值 0 → 手指/鼠标一动就进入拖拽
    //    · 标题区：阈值 6px → 避免与"点标题展开/收起"的点击冲突
    if (!drag.started) {
      const threshold = drag.grip ? 0 : 6
      if (Math.hypot(dx, dy) <= threshold) return
      beginDrag()
    }
    e.preventDefault()
    positionClone(e.clientX, e.clientY)
  }
  function onDragEnd() {
    if (!drag.active) return
    drag.active = false
    stopDragLoop()   // 停掉帧循环
    setFlipSuppressed(false)   // 恢复 FLIP 动画
    // 移除克隆体、还原占位条目样式
    if (drag.clone && drag.clone.parentNode) drag.clone.parentNode.removeChild(drag.clone)
    drag.clone = null
    draggingUid.value = null
    window.removeEventListener('pointermove', onDragMove)
    window.removeEventListener('pointerup', onDragEnd)
    window.removeEventListener('pointercancel', onDragEnd)
    // 真拖拽过 → 抑制随后触发的 collapse 标题点击（避免展开/收起误触发）；
    // 纯点击（未移动）则放行，让标题正常切换展开态。
    // ⚠ 关键：必须在 window 捕获阶段拦截（EP 的标题点击处理在冒泡阶段注册，
    //   绑在标题自身上会因同元素上的监听器按注册顺序执行而晚一步 → 拦不住），
    //   但只能拦截“落在按下时那个标题内”的点击：
    //   若无条件拦截下一次点击，用户拖完后再点“添加短语”按钮时这第一次点击会被吃掉
    //   → 表现为“要点两次才能添加”。
    if (drag.moved && drag.pressHeader) {
      const header = drag.pressHeader
      const stop = (ev) => {
        window.removeEventListener('click', stop, true)
        // 只在点击目标位于该标题内时才拦截；其它位置的点击（如“添加”按钮）一律放行
        if (header.contains(ev.target)) {
          ev.stopPropagation()
          ev.preventDefault()
        }
      }
      window.addEventListener('click', stop, true)
      // 兜底：若这次点击始终没来（如指针抬起在别处），300ms 后自动摘除，绝不长期占用
      setTimeout(() => window.removeEventListener('click', stop, true), 300)
    }
    drag.el = null
    drag.uid = null
    drag.fromIndex = -1
    drag.placeholderIndex = -1
    drag.moved = false
    drag.started = false
    drag.grip = false
    drag.pointerY = 0
    drag.lastFrameY = -1
    drag.lastTs = 0
    drag.rafId = 0
    drag.startTime = 0
    drag.intent = ''
    drag.cardBody = null
    drag.scrollers = []
    drag.pressHeader = null
    // 复原拖拽期间的全局样式开关（FLIP 抑制 + 关闭滚动锚定）
    setFlipSuppressed(false)
    setDraggingClass(false)
  }

  const filteredPhraseList = computed(() => {
    // 类型筛选已下沉到后端 load_phrases_page(char)，前端只展示后端已按类型过滤的当前页
    return allPhraseList.value
  })

  // 列表长度变化（首次加载 / 搜索 / 切类型 / 追加分页）后重算渐隐遮罩的显示与否
  watch(
    () => filteredPhraseList.value.length,
    () => { nextTick(updateListFade) }
  )

  // 搜索框输入 → 立即重新加载第一页（与切换类型一致：输入即查、即时转圈）
  // 后端查库很快，且 loadSeq 并发守卫会丢弃过期请求、lastLoadKey 去重避免重复查询
  watch(search_keyword_input_content, () => {
    if (suppressReload) return   // 窗口刷新期间清空搜索框不触发普通重载
    loadDataFromDb()
  })

  function handleCharClick(char) {
    activeChar.value = char
    search_keyword_input_content.value = ''
    loadDataFromDb() // 立即加载：点下去瞬间显示转圈，盖住查询耗时
  }

  // 本组件只监听自己需要的事件；这里不需要额外 listen，只在 onMounted 加载词条
  let unlisteners = []

  // 导入/导出完成后，App 通过 phraseDataVersion 自增触发本页刷新（强制重读）
  watch(phraseDataVersion, () => {
    if (suppressReload) return
    loadDataFromDb(true)
  })

  onMounted(async () => {
    // 安卓硬件返回键：添加/修改面板打开时，返回键等同点「取消」——
    // 走 closeAddPanel（添加模式会留存草稿，修改模式直接关闭），与面板自身按钮行为一致。
    // 路由切走（PhrasePage 卸载）时自动解绑，不会误关其他页面。
    unlisteners.push(bindBackClosable(add_content_panel_show, closeAddPanel))
    // 等待 App 启动期的 ensure_init 完成再加载词库：避免首次安装时 db 尚未建好/写入就读取导致空白。
    // 子组件 onMounted 先于父组件（App）执行，此时 appReady 仍为 false，故用 watch 延后首屏加载。
    if (appReady.value) {
      await loadDataFromDb()
    } else {
      const stop = watch(appReady, (v) => {
        if (v) { stop(); loadDataFromDb() }
      })
      // 兜底：极端情况下初始化迟迟未回调（如后端异常），最多等待 10s 强制加载，避免卡死空白
      setTimeout(() => {
        if (!appReady.value) { stop(); loadDataFromDb() }
      }, 10000)
    }
    // 窗口尺寸变化后重算底部渐隐遮罩与顶部留白互补
    const onResize = () => updateListFade()
    window.addEventListener('resize', onResize)
    unlisteners.push(() => window.removeEventListener('resize', onResize))
  })

  onUnmounted(() => {
    if (scrollRaf) cancelAnimationFrame(scrollRaf)
    // 组件卸载时若还在拖拽，必须停掉帧循环 + 摘掉全局指针监听，否则 rAF 会一直跑
    drag.active = false
    stopDragLoop()
    setFlipSuppressed(false)
    setDraggingClass(false)
    window.removeEventListener('pointermove', onDragMove)
    window.removeEventListener('pointerup', onDragEnd)
    window.removeEventListener('pointercancel', onDragEnd)
    unlisteners.forEach(fn => fn())
  })

  return {
    // 模板用到的状态与方法
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
    // 懒加载 / 滑动窗口
    allPhraseList,
    aboveCount,
    listScrollRef,
    formScroll,
    onListScroll,
    listHasMoreBelow,
    loadingMore,
    hasMore,
    loading,
    refreshMask,
    // 空状态
    isSearching,
    emptyDesc,
    Search,
    // 供 App.vue 调用
    loadDataFromDb,
  }
}
