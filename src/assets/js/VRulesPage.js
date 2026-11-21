// ============================================================
// VRulesPage.js —— VRulesPage.vue 的逻辑组合式函数
// 发音规则页：
//   · 左侧纯圆形导航轨（无分组），与 PhrasePage 字母轨同构
//   · 顶部搜索框：只按「字母名」过滤（如 a / th / ed），不搜规则正文
//   · scrollspy：滚动到哪一段，左侧对应锚点就高亮到哪（并自动滚入可视区）
//   · 点击跳转期间锁定 scrollspy，避免左轨跟随平滑滚动「瞎滚」
// ============================================================

import { ref, computed, watch, nextTick } from 'vue'

// 圆形导航轨数据：id 对应内容区 <section id>，short 是圆形按钮里显示的字母，
// group 是分组标题（两字关键词，与右侧 5 个大标题一一对应）
const G_BASIC = '基础'
const G_VOWEL = '元音'
const G_CONS = '辅音'
const G_COMB = '组合'
export const NAV_ITEMS = [
  { id: 'sec-basic', label: '语音基础', short: '#', group: G_BASIC },
  { id: 'ltr-a', label: 'a', short: 'a', group: G_VOWEL },
  { id: 'ltr-e', label: 'e', short: 'e', group: G_VOWEL },
  { id: 'ltr-i', label: 'i', short: 'i', group: G_VOWEL },
  { id: 'ltr-o', label: 'o', short: 'o', group: G_VOWEL },
  { id: 'ltr-u', label: 'u', short: 'u', group: G_VOWEL },
  { id: 'ltr-b', label: 'b', short: 'b', group: G_CONS },
  { id: 'ltr-c', label: 'c', short: 'c', group: G_CONS },
  { id: 'ltr-g', label: 'g', short: 'g', group: G_CONS },
  { id: 'ltr-h', label: 'h', short: 'h', group: G_CONS },
  { id: 'ltr-k', label: 'k', short: 'k', group: G_CONS },
  { id: 'ltr-n', label: 'n', short: 'n', group: G_CONS },
  { id: 'ltr-s', label: 's', short: 's', group: G_CONS },
  { id: 'ltr-t', label: 't', short: 't', group: G_CONS },
  { id: 'ltr-w', label: 'w', short: 'w', group: G_CONS },
  { id: 'ltr-ar', label: 'ar', short: 'ar', group: G_COMB },
  { id: 'ltr-ch', label: 'ch', short: 'ch', group: G_COMB },
  { id: 'ltr-ea', label: 'ea', short: 'ea', group: G_COMB },
  { id: 'ltr-ear', label: 'ear', short: 'ear', group: G_COMB },
  { id: 'ltr-ed', label: 'ed', short: 'ed', group: G_COMB },
  { id: 'ltr-ere', label: 'ere', short: 'ere', group: G_COMB },
  { id: 'ltr-ex', label: 'ex', short: 'ex', group: G_COMB },
  { id: 'ltr-gh', label: 'gh', short: 'gh', group: G_COMB },
  { id: 'ltr-oo', label: 'oo', short: 'oo', group: G_COMB },
  { id: 'ltr-or', label: 'or', short: 'or', group: G_COMB },
  { id: 'ltr-our', label: 'our', short: 'our', group: G_COMB },
  { id: 'ltr-ow', label: 'ow', short: 'ow', group: G_COMB },
  { id: 'ltr-sion', label: 'sion', short: 'sion', group: G_COMB },
  { id: 'ltr-th', label: 'th', short: 'th', group: G_COMB },
  { id: 'ltr-tion', label: 'tion', short: 'tion', group: G_COMB },
  { id: 'ltr-wh', label: 'wh', short: 'wh', group: G_COMB },
]

export function useVRulesPage() {
  const search = ref('')
  const activeId = ref('sec-basic')
  const scrollRef = ref(null)

  // 搜索词（小写、去空格）：只按字母名匹配
  const keyword = computed(() => (search.value || '').trim().toLowerCase())

  // 搜索无结果时的占位锚点：左侧只留一个空按钮，不对应任何内容 section
  const EMPTY_NAV_ID = '__vrules_empty__'
  const noMatch = computed(() => keyword.value.length > 0 && visibleItems.value.length === 0)

  // 字母名是否可见：§ 语音基础不属于字母，一旦开始搜索就隐藏
  function isVisible(letter) {
    const q = keyword.value
    if (!q) return true
    if (letter === '#') return false
    return String(letter).toLowerCase().includes(q)
  }

  // 左侧圆形轨：随搜索过滤（v-for 用）
  const visibleItems = computed(() =>
    NAV_ITEMS.filter((it) => it.id === 'sec-basic' ? !keyword.value : isVisible(it.label))
  )

  // 按分组聚合：搜索时只保留仍有可见项的分组，分组标题本身用两字关键词。
  // 完全匹配不到时，不再让左轨全空，而是保留一个无文字、无跳转的空按钮。
  const GROUP_LABELS = [G_BASIC, G_VOWEL, G_CONS, G_COMB]
  const groupedItems = computed(() => {
    if (noMatch.value) {
      return [{
        label: '',
        items: [{ id: EMPTY_NAV_ID, label: '', short: '', group: '', empty: true }],
      }]
    }
    return GROUP_LABELS.map((label) => ({
      label,
      items: visibleItems.value.filter((it) => it.group === label),
    })).filter((grp) => grp.items.length > 0)
  })

  // ---- 让指定高亮项滚进左侧轨可视区（不在视口内就自动上/下滚）----
  function keepActiveVisible(id) {
    const track = document.querySelector('.vrules-type')
    if (!track) return
    // 点击跳转时 activeId 已更新、但 Vue 尚未刷新 li.active class，
    // 所以优先按 data-id 精确定位目标，避免拿到旧的高亮项。
    const active = (id && track.querySelector('li[data-id="' + id + '"]'))
                || track.querySelector('li.active')
    if (!active) return
    const tr = track.getBoundingClientRect()
    const ar = active.getBoundingClientRect()
    let target = null
    if (ar.top < tr.top + 4) {
      target = track.scrollTop - ((tr.top - ar.top) + 8)
    } else if (ar.bottom > tr.bottom - 4) {
      target = track.scrollTop + ((ar.bottom - tr.bottom) + 8)
    }
    if (target !== null) {
      track.scrollTo({ top: Math.max(0, target), behavior: 'smooth' })
    }
  }

  // ---- 点击跳转锁：平滑滚动期间暂停 scrollspy，左轨不跟随主体逐段滚 ----
  let suppressSpy = false
  let settleTimer = null
  function armSettle() {
    clearTimeout(settleTimer)
    // 平滑滚动每帧都会触发 scroll，timer 不断重置；静止 180ms 后恢复 scrollspy
    settleTimer = setTimeout(() => { suppressSpy = false }, 180)
  }

  // 搜索变化后：高亮落到第一个可见项，内容回顶部，左轨同步定位。
  // 无匹配时高亮落到空按钮，避免旧锚点残留高亮。
  watch(keyword, async () => {
    await nextTick()
    const first = visibleItems.value[0]
    if (first) {
      activeId.value = first.id
      keepActiveVisible(first.id)
    } else if (noMatch.value) {
      activeId.value = EMPTY_NAV_ID
    }
    if (scrollRef.value) scrollRef.value.scrollTop = 0
  })

  // 点击圆形锚点：内容平滑滚到对应 section；左轨一次性定位，不跟随滚动过程。
  // 无匹配时的空按钮不对应任何 section，直接忽略点击。
  function goAnchor(id) {
    if (id === EMPTY_NAV_ID) return
    activeId.value = id
    suppressSpy = true
    const box = scrollRef.value
    if (box) {
      const el = box.querySelector('#' + id)
      if (el) box.scrollTo({ top: el.offsetTop - 8, behavior: 'smooth' })
    }
    keepActiveVisible(id)
    armSettle() // 兜底：目标就在眼前、没有 scroll 事件时也能解锁
  }

  // scrollspy：滚动到哪一段，锚点就高亮到哪（rAF 节流）
  let ticking = false
  function onScroll() {
    if (ticking) return
    ticking = true
    requestAnimationFrame(() => {
      ticking = false
      if (suppressSpy) { armSettle(); return } // 点击跳转中：左轨锁定
      if (noMatch.value) { activeId.value = EMPTY_NAV_ID; return } // 无匹配：保持空按钮高亮
      const box = scrollRef.value
      if (!box) return
      const threshold = box.scrollTop + 60
      let current = visibleItems.value[0]?.id || null
      for (const item of visibleItems.value) {
        const el = box.querySelector('#' + item.id)
        if (el && el.offsetTop <= threshold) current = item.id
        else break
      }
      if (current && current !== activeId.value) activeId.value = current
      keepActiveVisible(current) // 每次滚动都校正，锚点出视口就自动上/下滚
    })
  }

  function clearSearch() {
    search.value = ''
  }

  return {
    search, activeId, scrollRef, visibleItems, groupedItems, noMatch,
    isVisible, goAnchor, onScroll, clearSearch,
  }
}
