// ============================================================
// PhrasePage.js —— PhrasePage.vue 的逻辑组合式函数
// 这里集中存放 PhrasePage 的所有状态与业务逻辑，供 .vue 的
// <script setup> 通过 usePhrasePage() 解构使用。
// 包含：词条增删改、重复关键词“追加”对话框、分页懒加载
//   （启动时加载 PAGE_SIZE 条，滚到底再加载下一页）。
// ============================================================

// Tauri
import { invoke } from "@tauri-apps/api/core";
import { listen } from '@tauri-apps/api/event'
// 全局共享状态
import { add_content_panel_show, phraseDataVersion, appReady } from "./index.js"
// Element Plus（图标已在 main.js 全局注册，这里只需消息框）
import { ElMessage, ElMessageBox } from 'element-plus'
import { Search } from '@element-plus/icons-vue'
// Vue
import { reactive, ref, onMounted, nextTick, computed, toRaw, onUnmounted, watch } from 'vue'

const PAGE_SIZE = 100

export function usePhrasePage() {
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
    list: [{ phrase: '', translate: '' }]
  })
  const activeNames = ref(['1'])

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
  const loadedCount = ref(0)
  const totalCount = ref(0)
  const loadingMore = ref(false)
  const hasMore = computed(() => loadedCount.value < totalCount.value)
  // 切换类型 / 搜索时的整体加载遮罩（转圈）
  const loading = ref(false)

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
      form.list = [{ phrase: '', translate: '' }]
      activeNames.value = ['1']
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
    form.list = item.list.map(p => ({ ...p }))
    // 编辑时把所有短语条目全部展开
    activeNames.value = form.list.map((_, i) => String(i + 1))
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
  async function fetchPage(offset) {
    const kw = currentSearchKeyword()
    if (kw) {
      return await invoke('search_phrases', { keyword: kw, offset, limit: PAGE_SIZE })
    }
    return await invoke('load_phrases_page', { char: activeChar.value, offset, limit: PAGE_SIZE })
  }

  // 加载第一页（重置分页）；导入/导出完成后也走这里刷新
  // force=true 时忽略去重，强制刷新（导入/导出数据已变，即使查询条件相同也要重读）
  // preserveScroll=true 时刷新后保持原来的滚动位置（增删改属于原地更新，不希望跳回顶部）
  let lastLoadKey = ''
  let loadSeq = 0
  async function loadDataFromDb(force = false, preserveScroll = false) {
    const kw = currentSearchKeyword()
    const key = kw ? `s:${kw}` : `b:${activeChar.value}`
    // 去重：相同查询条件不重复加载（避免切换类型清空搜索框后又触发一次多余加载）
    if (!force && key === lastLoadKey) return
    lastLoadKey = key
    const seq = ++loadSeq
    // 记录当前滚动位置（清空前），用于“原地刷新”后恢复
    let savedScrollTop = 0
    const scrollEl = listScrollRef.value
    if (preserveScroll && scrollEl) savedScrollTop = scrollEl.scrollTop
    loading.value = true
    allPhraseList.value = []
    loadedCount.value = 0
    totalCount.value = 0
    try {
      const res = await fetchPage(0)
      // 若期间又发起了新的加载，丢弃这次过期结果，避免旧数据覆盖新数据
      if (seq !== loadSeq) return
      const rows = res.rows || []
      allPhraseList.value = rows.map(parseEntry)
      loadedCount.value = allPhraseList.value.length
      totalCount.value = res.total ?? 0
      show_data.value = allPhraseList.value.length === 0
    } catch (err) {
      console.error("启动加载数据库失败", err)
      ElMessage({
        message: `加载数据失败：${err}`,
        type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause'
      })
    } finally {
      // 仅当本次仍是“最新一次加载”时才收起转圈，避免被更早的请求误关
      if (seq === loadSeq) loading.value = false
      // 原地刷新：等 DOM 重绘后再恢复滚动位置
      if (preserveScroll && seq === loadSeq) {
        await nextTick()
        // 若之前已懒加载了更多分页、滚动位置较深，刷新只重取了第一页，
        // 内容高度不足以支撑原 scrollTop，需要继续补齐分页直到够高再恢复
        while (hasMore.value && scrollEl && scrollEl.scrollHeight < savedScrollTop + scrollEl.clientHeight) {
          await loadMore()
          await nextTick()
        }
        if (scrollEl) {
          const maxTop = scrollEl.scrollHeight - scrollEl.clientHeight
          scrollEl.scrollTop = Math.min(savedScrollTop, Math.max(0, maxTop))
        }
      }
    }
  }

  // 滚到底：加载下一页并追加到列表
  async function loadMore() {
    if (loadingMore.value) return
    if (loadedCount.value >= totalCount.value) return
    loadingMore.value = true
    try {
      const res = await fetchPage(loadedCount.value)
      const rows = res.rows || []
      const parsed = rows.map(parseEntry)
      allPhraseList.value.push(...parsed)
      loadedCount.value = allPhraseList.value.length
      totalCount.value = res.total ?? totalCount.value
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
    el.style.marginTop = top + 'px'
    el.style.paddingTop = (12 - top) + 'px'
  }

  function onListScroll() {
    const el = listScrollRef.value
    if (!el) return
    updateListFade()
    // 距底部 80px 内即触发加载下一页
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 80) {
      loadMore()
    }
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
        list: JSON.stringify(raw.list),
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
        form.list = [{ phrase: '', translate: '' }]
        activeNames.value = ['1']
        savedAddState.value = {
          type: form.type,
          keyword: '',
          first_char: '',
          list: [{ phrase: '', translate: '' }],
          activeNames: ['1'],
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
          loadDataFromDb(true, true)
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
    const listStr = JSON.stringify(raw.list)
    // 编辑模式下改了关键词触发重复：记下面板打开时的原词条关键词，用于先删原行
    const editingKeyword = editingKeyWord.value

    // 1) 乐观更新：追加完成保留面板（便于继续录入），仅复位表单内容，不等后端写入
    formMode.value = 'add'
    editingKeyWord.value = ''
    form.keyword = ''
    form.first_char = ''
    form.list = [{ phrase: '', translate: '' }]
    activeNames.value = ['1']
    savedAddState.value = {
      type: form.type,
      keyword: '',
      first_char: '',
      list: [{ phrase: '', translate: '' }],
      activeNames: ['1'],
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
        loadDataFromDb(true, true)
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
    // 删除后保持剩余条目全部展开
    activeNames.value = form.list.map((_, i) => String(i + 1))
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
    form.list.push({ phrase: '', translate: '' })
    const newIndex = form.list.length
    activeNames.value.push(String(newIndex))
    await nextTick()
    // 点击当下立即滚到新条目（即时，无延迟）；折叠面板约 300ms 展开动画期间高度会继续增长，
    // 动画结束后再无感校正一次到底部，避免新条目被撑在可视区之外
    scrollNewItemIntoView()
    setTimeout(scrollNewItemIntoView, 320)
  }
  // 让最后一个短语条目平滑滚入视口底部（由浏览器自动找最近的滚动祖先 .demo-collapse）
  function scrollNewItemIntoView() {
    const wrap = formScroll.value
    if (!wrap) return
    const boxes = wrap.querySelectorAll('.phrase-content-box')
    const last = boxes[boxes.length - 1]
    if (last) last.scrollIntoView({ block: 'end', behavior: 'smooth' })
  }
  const handleChange = (val) => {
    console.log(val)
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
    loadDataFromDb(true)
  })

  onMounted(async () => {
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
    formMode,
    keywordInput,
    clear_keyword_content,
    openAddForm,
    // 懒加载
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
    // 供 App.vue 调用
    loadDataFromDb,
  }
}
