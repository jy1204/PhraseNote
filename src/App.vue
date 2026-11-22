<template>
  <main class="container" @dragover.prevent @drop.prevent>
    <!-- 顶部菜单栏：文件｜互传｜设置｜帮助（原生实现，替代 Reka‑UI，功能/样式保持一致） -->
    <div class="menu-bar">
      <div class="menu-group">
        <div
          class="menu-options"
          :class="{ open: currentMenu === 'file' }"
          @click.stop="toggleMenu('file')"
        >文件</div>
        <div v-if="currentMenu === 'file'" class="menu-list">
          <div class="menu-list-item" @click="onMenuItem('file', openImportDialog)">导入</div>
          <div class="menu-list-item" @click="onMenuItem('file', openExportDialog)">导出</div>
        </div>
      </div>
      <div class="menu-group">
        <div
          class="menu-options"
          :class="{ open: currentMenu === 'transfer' }"
          @click.stop="toggleMenu('transfer')"
        >互传</div>
        <div v-if="currentMenu === 'transfer'" class="menu-list">
          <div class="menu-list-item" @click="onMenuItem('transfer', openSendPanel)">发送</div>
          <div class="menu-list-item" @click="onMenuItem('transfer', openReceivePanel)">接收</div>
        </div>
      </div>
      <div class="menu-group">
        <div class="menu-options" @click.stop="openSettings">设置</div>
      </div>
      <div class="menu-group">
        <div
          class="menu-options"
          :class="{ open: currentMenu === 'help' }"
          @click.stop="toggleMenu('help')"
        >帮助</div>
        <div v-if="currentMenu === 'help'" class="menu-list">
          <div class="menu-list-item" @click="onMenuItem('help', openBatchPhrase)">批处理短语</div>
        </div>
      </div>
      <!-- 右侧竖三点：更多操作，当前提供「全局搜索」（应用内搜索框，效果对齐 Ctrl+F） -->
      <div class="menu-group menu-more">
        <div
          class="menu-options menu-more-btn"
          :class="{ open: currentMenu === 'more' }"
          title="更多"
          @click.stop="toggleMenu('more')"
        >
          <span class="menu-more-dots" aria-hidden="true"><i></i><i></i><i></i></span>
        </div>
        <div v-if="currentMenu === 'more'" class="menu-list menu-more-list">
          <div class="menu-list-item" @click="onMenuItem('more', openFind)">全局搜索</div>
        </div>
      </div>

      <!-- 【全局搜索】应用内查找栏：对齐 Ctrl+F 的高亮 / 匹配计数 / 上一下一个 -->
      <transition name="pn-find">
      <div
        v-if="findShow"
        class="pn-find-bar"
        :class="{ 'is-dragging': findDragging }"
        :style="findBarStyle"
        @pointerdown="onFindBarPointerDown"
        @pointermove="onFindBarPointerMove"
        @pointerup="onFindBarPointerUp"
        @pointercancel="onFindBarPointerUp"
        @contextmenu.prevent
      >
        <el-input
          ref="findInputRef"
          v-model="findQuery"
          class="pn-find-input"
          placeholder="在当前页面查找"
          :prefix-icon="Search"
          clearable
          @input="onFindInput"
          @clear="onFindClear"
          @keyup.enter.exact="onFindNext"
          @keyup.enter.shift="onFindPrev"
        />
        <span class="pn-find-count">{{ findCount ? `${findCurrent} / ${findCount}` : (findQuery ? '无结果' : '') }}</span>
        <button class="pn-find-nav" title="上一个" :disabled="!findCount" @click="onFindPrev">‹</button>
        <button class="pn-find-nav" title="下一个" :disabled="!findCount" @click="onFindNext">›</button>
        <button class="pn-find-close" title="关闭搜索" @click="closeFind">✕</button>
      </div>
      </transition>
    </div>

    <!-- 【发送文件面板】局域网扫描设备，选中设备发送词库 -->
    <transition name="pn-overlay">
    <div id="send-file" v-show="send_file_show" @click.self="closeSendPanel">
      <div class="transmission-panel">
        <button id="close-send-file" @click="closeSendPanel">×</button>
        <dl>
          <dt><h2>附近的设备：</h2></dt>
          <dd v-for="dev in deviceList" :key="dev.id"
              :class="{ selected: selectedDeviceIds.includes(dev.id) }"
              @click="toggleDevice(dev.id)">
            <span>{{ dev.name }}</span>
            <span class="dev-ip">{{ dev.ip }}:{{ dev.port }}</span>
          </dd>
          <dd v-if="deviceList.length === 0" class="empty-dev">未发现设备</dd>
        </dl>
        <p>请确保目标连接的是同一个Wi‑Fi网络</p>
        <div class="transmission-actions">
          <el-button type="primary" @click="doSend">发送到选中设备</el-button>
        </div>
      </div>
    </div>
    </transition>

    <!-- 【接收文件面板】按发送设备分组：点击设备=选中（高亮），底部「导入」按钮触发导入；按按钮后先关面板再导入 -->
    <transition name="pn-overlay">
    <div id="receive-file" v-show="receive_file_show" @click.self="closeReceivePanel">
      <div class="transmission-panel">
        <button id="close-receive-file" @click="closeReceivePanel">×</button>
        <dl>
          <dt><h2>收到的文件（按设备）：</h2></dt>
          <dd v-for="grp in deviceGroups" :key="grp.device"
              class="rf-device-row"
              :class="{ selected: selectedDevices.includes(grp.device) }"
              @click="toggleDeviceSelect(grp.device)">
            <span class="rf-device">来自：{{ grp.device }}</span>
            <span class="rf-count">{{ grp.files.length }} 个文件 · 点击选择</span>
          </dd>
          <dd v-if="receivedFiles.length === 0" class="empty-dev">暂无收到的文件</dd>
        </dl>
        <p>请确保目标连接的是同一个Wi‑Fi网络</p>
        <div class="transmission-actions receive-actions">
          <el-button type="primary" :disabled="selectedDevices.length === 0" @click="onImportButtonClick">导入</el-button>
        </div>
      </div>
    </div>
    </transition>

    <!-- 全局全屏Loading遮罩：导出、导入、发送、导入接收文件时显示 -->
    <div v-if="loading_show" class="loading-mask">
      <div class="loading-spinner"></div>
      <p>{{ loading_text }}</p>
      <!-- 导入任务才显示取消按钮 -->
      <el-button v-if="importing && !dupResolving" type="danger" @click="cancelImport" style="margin-top:16px">取消</el-button>
    </div>

    <!-- 启动期全局初始化遮罩：App 首帧即显示，ensure_init 完成后由 init_show 关闭。
         复用 .loading-mask 样式（不显示取消按钮，importing 默认 false）。 -->
    <div v-if="init_show" class="loading-mask">
      <div class="loading-spinner"></div>
      <p>正在初始化数据，请稍候…</p>
    </div>

    <!-- 主业务外壳 -->
    <div class="main-board">
      <div class="option-panel" ref="optionPanelRef">
        <ul>
          <li
            v-for="item in navItems"
            :key="item.path"
            :class="{ active: route.path === item.path }"
            @click="onNavClick(item.path)"
          >
            <el-icon class="nav-icon" :size="18"><component :is="item.icon" /></el-icon>
            <span class="nav-label">{{ item.label }}</span>
          </li>
        </ul>
      </div>
      <!-- 词条内容组件：通过 vue-router 渲染 PhrasePage / NotePage；
           切换动画：淡入 + 轻微上移缩放（out-in：旧页先离场再入场，无布局跳动）。
           v-slot + :key 写法保证按路由路径可靠触发过渡 -->
      <router-view v-slot="{ Component }">
        <transition name="pn-page" mode="out-in" appear @after-enter="onFindPageEntered">
          <component :is="Component" :key="route.path" />
        </transition>
      </router-view>
    </div>

    <!-- 【重复关键词冲突弹窗】导入zip / 导入接收文件遇到重复关键词弹出 -->
    <div v-if="dupDialogShow" class="dup-mask">
      <div class="dup-dialog">
        <h2>发现重复关键词</h2>
        <p class="dup-kw">关键词：<b>{{ currentDup?.keyword }}</b></p>
        <div class="dup-diff">
          <div class="dup-label">原有内容：</div>
          <pre>{{ currentDup?.old_list }}</pre>
          <div class="dup-label">新内容：</div>
          <pre>{{ currentDup?.new_list }}</pre>
        </div>
        <el-checkbox v-model="dupRemember" :disabled="dupResolving">下次遇到此情况直接执行</el-checkbox>
        <div class="dup-actions">
          <el-button :disabled="dupResolving" @click="resolveCurrent('skip')">跳过</el-button>
          <el-button :loading="dupResolvingAction === 'append'" :disabled="dupResolving" @click="resolveCurrent('append')">追加</el-button>
          <el-button type="primary" :loading="dupResolvingAction === 'overwrite'" :disabled="dupResolving" @click="resolveCurrent('overwrite')">覆盖</el-button>
          <el-button :disabled="dupResolving" @click="cancelDupImport">取消导入</el-button>
        </div>
      </div>
    </div>

    <!-- 【设置窗口】带遮罩：第一栏选项（外观），第二栏内容（外观设置）。桌面竖排/移动横排由对应 css 控制；
         用 <transition name="pn-overlay"> 包裹，关闭时复用打开动画倒放一次，与发送/接收/添加表单保持一致 -->
    <transition name="pn-overlay">
    <div v-if="settings_show" class="settings-mask" @click.self="closeSettings">
      <div class="settings-window">
        <div class="settings-body">
          <div class="settings-nav">
            <div
              v-for="opt in settingsOptions"
              :key="opt.key"
              class="settings-nav-item"
              :class="{ active: currentSetting === opt.key }"
              @click="currentSetting = opt.key"
            >{{ opt.label }}</div>
          </div>
          <div class="settings-content">
            <div v-if="currentSetting === 'appearance'">
              <div class="appearance-row">
                <span class="appearance-label">外观设置</span>
                <el-select
                  v-model="theme"
                  class="appearance-select"
                  popper-class="settings-select-popper"
                  @change="selectTheme(theme)"
                >
                  <el-option
                    v-for="t in themeOptions"
                    :key="t.value"
                    :label="t.label"
                    :value="t.value"
                  />
                </el-select>
              </div>
              <div class="appearance-row">
                <span class="appearance-label">主题配色</span>
                <el-select
                  v-model="scheme"
                  class="appearance-select"
                  popper-class="settings-select-popper"
                  @change="selectScheme(scheme)"
                >
                  <el-option
                    v-for="s in schemeOptions"
                    :key="s.value"
                    :label="s.label"
                    :value="s.value"
                  />
                </el-select>
              </div>
            </div>
            <div v-if="currentSetting === 'update'">
              <div class="appearance-row">
                <span class="appearance-label">当前版本</span>
                <div class="update-version-actions">
                  <span class="update-current-version">v{{ appVersion }}</span>
                  <el-button size="small" @click="checkForUpdate(true)">检查更新</el-button>
                </div>
              </div>
              <div class="appearance-row">
                <span class="appearance-label">启动时检查更新</span>
                <el-switch v-model="checkUpdateOnStart" @change="selectCheckOnStart" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    </transition>

    <!-- 【批处理短语】全局遮罩浮窗（帮助菜单）：复制提示词给 AI 生成 → 粘贴结果到文本框 → 录入（后端单命令单事务合并入库） -->
    <transition name="pn-overlay">
    <div v-if="batch_phrase_show" class="batch-mask" @click.self="closeBatchPhrase" @dragover.prevent="onBatchDragOver" @drop.prevent="onBatchDrop">
      <div class="batch-window">
        <button class="batch-close" @click="closeBatchPhrase">×</button>
        <div class="batch-header">
          <h2 class="batch-title">批处理短语</h2>
          <p class="batch-subtitle">批量录入的快捷入口：提示词 → 生成 → 粘贴 → 录入</p>
        </div>
        <div class="batch-body">
          <div class="batch-tip">
            <div class="batch-tip-text">
              <p>点击按钮复制提示词，发给 AI，让它按格式生成短语，再把结果粘贴到下方文本框；也可以选取本地 JSON 文件载入，或直接拖入本窗口（均支持多选）。</p>
            </div>
            <div class="batch-tip-actions">
              <button class="batch-file-btn" @click="pickJsonFiles">选取JSON文件</button>
              <button class="batch-copy-btn" @click="copyBatchPrompt">复制提示词</button>
              <!-- 隐藏的原生文件选择框：仅通过上方按钮触发，多选 .json -->
              <input
                ref="json_file_input"
                type="file"
                accept=".json,application/json"
                multiple
                class="batch-file-input"
                @change="onJsonFileChange"
              />
            </div>
          </div>
          <textarea
            v-model="batch_input"
            class="batch-input"
            placeholder="在此粘贴 AI 生成的短语内容"
          ></textarea>
          <!-- 录入失败明细（有失败条目时才显示，保留原文便于修正重试） -->
          <div v-if="batch_failures.length" class="batch-failures">
            <div class="batch-failures-title">以下条目未能录入：</div>
            <div v-for="(f, i) in batch_failures" :key="i" class="batch-failures-item">
              <span class="batch-failures-keyword">{{ f.keyword || '（无关键词）' }}</span>
              <span class="batch-failures-reason">{{ f.reason }}</span>
            </div>
          </div>
          <div class="batch-actions">
            <button class="batch-entry-btn" :disabled="batch_entry_loading" @click="doBatchEntry">
              {{ batch_entry_loading ? '录入中…' : '录入' }}
            </button>
          </div>
        </div>
        <!-- 拖放识别（电脑端）：拖入文件时高亮整个浮窗，松手即按「选取文件」同款流程解析载入 -->
        <div v-if="batch_drag_over" class="batch-drop-hint">
          <svg class="batch-drop-hint-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
          <div class="batch-drop-hint-text">松开载入 JSON 文件</div>
          <div class="batch-drop-hint-sub">支持一次拖入多个文件</div>
        </div>
      </div>
    </div>
    </transition>

    <!-- 整屏编辑视图：选中笔记时以 position:fixed 全屏覆盖整个 app（含菜单栏与侧边导航），点返回即关闭；
         开合动画：上滑淡入 / 下滑淡出（覆盖层为 fixed 全屏，无需 out-in） -->
    <transition
      name="pn-editor"
      appear
      @after-enter="onFindPageEntered"
      @after-leave="onFindPageEntered"
    >
      <NoteEditor v-if="selectedNote" />
    </transition>

    <!-- 冷更新：发现新版本提示弹窗（启动时后端检测，失败静默不弹） -->
    <el-dialog
      v-model="updateDialogShow"
      title="发现新版本"
      width="min(420px, calc(100vw - 32px))"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      :show-close="true"
      class="pn-update-dialog"
      modal-class="pn-update-overlay"
      @closed="onUpdateDialogClosed"
    >
      <div v-if="!updateDownloading" class="pn-update-info">
        <div class="pn-update-version">新版本 {{ updateInfo.version }}</div>
        <div class="pn-update-notes">{{ updateInfo.notes || '暂无更新说明' }}</div>
        <!-- 安装阶段的问题就在弹窗里说清楚：长文案（系统异常串 / 安装包路径）会正常换行，
             不再塞进一行 toast 把提示框撑爆 -->
        <div v-if="updateError" class="pn-update-alert pn-update-alert--error">{{ updateError }}</div>
        <div v-else-if="updatePermissionTip" class="pn-update-alert">{{ updatePermissionTip }}</div>
      </div>
      <div v-else class="pn-update-progress">
        <div class="pn-update-progress-label">
          {{ updateProgress >= 100 ? (updateNeedPermission ? '准备安装…' : '下载完成，正在安装…') : '正在下载安装包…' }}
        </div>
        <el-progress :percentage="updateProgress" :stroke-width="14" />
        <div class="pn-update-progress-actions">
          <el-button @click="cancelDownload">取消下载</el-button>
          <el-button type="primary" @click="backgroundDownload">后台下载</el-button>
        </div>
      </div>
      <template #footer>
        <div v-if="!updateDownloading" class="pn-update-footer">
          <el-button @click="cancelUpdate">{{ updateError || updateNeedPermission ? '关闭' : '取消' }}</el-button>
          <!-- 还差「安装未知应用」授权：安装包已经在本地，直接复用，不重新下载 -->
          <el-button v-if="updateNeedPermission" type="primary" @click="installPending">继续安装</el-button>
          <el-button v-else-if="updateError" type="primary" @click="startUpdate">重试</el-button>
          <el-button v-else type="primary" @click="startUpdate">更新</el-button>
        </div>
      </template>
    </el-dialog>
  </main>
</template>

<script setup lang="ts">
/// <reference types="vite/client" />
/// <reference types="element-plus/global" />
// Tauri 核心调用、事件监听API
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
// 安卓硬件返回键（Tauri 2.9+ 内置）：监听后系统不再走默认行为，由我们统一分发
import { onBackButtonPress, exit, getVersion } from '@tauri-apps/api/app'
// 返回键「可关闭界面」栈：界面打开入栈、关闭出栈，返回键关掉最上面一个
import { bindBackClosable, consumeBackPress } from './assets/js/backStack.js'
// 全局搜索（应用内查找栏，效果对齐 Ctrl+F）：DOM 文本高亮 + 匹配计数 + 上/下一个
import { createFindInPage } from './assets/js/findInPage.js'
import { Search } from '@element-plus/icons-vue'
// Tauri 文件对话框：保存、打开文件
import { save, open } from '@tauri-apps/plugin-dialog'
// 外部状态
import { currentMenu, phraseDataVersion, noteDataVersion, selectedNote, appReady } from "./assets/js/index.js"
// 全屏笔记编辑视图：选中文件时整屏覆盖原 app（菜单栏 + 侧边导航）
import NoteEditor from "./assets/views/NoteEditor.vue"

// -------------------- 顶部菜单栏（原生实现，替代 Reka‑UI）--------------------
// currentMenu 来自外部状态：'' 表示全部收起，'file' / 'transfer' / 'help' 表示展开对应菜单
function toggleMenu(name: string) {
  currentMenu.value = currentMenu.value === name ? '' : name
}
// 点击菜单项：先执行对应功能，再收起菜单
function onMenuItem(_name: string, fn: () => void) {
  fn()
  currentMenu.value = ''
}
// 下拉菜单贴到屏幕左右边缘时自动往回平移，保证整块菜单可见
watch(currentMenu, () => {
  if (!currentMenu.value) return
  nextTick(() => {
    const list = document.querySelector('.menu-bar .menu-list') as HTMLElement | null
    if (!list || !list.parentElement) return
    const lr = list.getBoundingClientRect()
    const gr = list.parentElement.getBoundingClientRect()
    const margin = 8
    let dx = 0
    if (lr.right > window.innerWidth - margin) dx = window.innerWidth - margin - lr.right
    else if (lr.left < margin) dx = margin - lr.left
    if (!dx) return
    list.style.left = `${lr.left + dx - gr.left}px`
    list.style.right = 'auto'
  })
})

// -------------------- 全局搜索（应用内查找栏）--------------------
const findShow = ref(false)
const findQuery = ref('')
const findCount = ref(0)
const findCurrent = ref(0)
const findInputRef = ref<any>(null)
const finder = createFindInPage()
// 笔记保存时的查找处理：
// - CSS Highlight 模式（正常路径）根本不碰编辑器 DOM，getHtml() 不会读走高亮，
//   所以什么都不用做，保存后高亮/计数原样保留。
// - 老 WebView 降级到 <mark> 模式时，暂时撤掉 mark 避免被 getHtml() 读走，
//   保存后（editor.getHtml() 之后）用原词重建，用户不会看到"保存后无结果"。
// 返回一个恢复函数，由编辑器在取完 html 后立即调用。
provide('pnBeforeSaveFind', () => {
  const active = findShow.value && !!findQuery.value.trim() && !!selectedNote.value
  if (!active || finder.getMode() !== 'mark') return () => {}
  finder.clear()
  return () => { refreshFind(true, true) }
})
// 搜索范围：主业务外壳 .main-board；findInPage 内部再排除选项卡、左侧导航轨、
// 各页面搜索框与查找栏本身，只高亮正文内容
function findRoot(): HTMLElement | null {
  // 打开笔记时编辑视图是整屏浮层，查找范围只限当前这一页（编辑器本身），
  // 不搜底部被遮住的列表页；关闭后回到主业务外壳。
  // 编辑视图只把正文编辑区算进去：标题（含跑马灯副本）、工具栏、保存按钮都不参与查找。
  if (selectedNote.value) return document.querySelector('.note-editor .editor-body') as HTMLElement | null
  return document.querySelector('.main-board') as HTMLElement | null
}
let findObserver: MutationObserver | null = null
// 监听器当前挂在哪个根上：查找根会随「列表 ↔ 编辑视图」切换而变，
// 仅凭 findObserver 是否存在判断是不够的（老监听器还挂在被遮住的列表页上）。
let findObservedRoot: HTMLElement | null = null
let findRecalcTimer: number | null = null
let suppressFindObserver = false
// 上次重查时查找根的纯文本：<mark> 的插入/还原不会改变 textContent，
// 用它判断"内容真的变了"，避免编辑器与查找互相触发重算的死循环。
let findRootText = ''
// 切换选项卡期间暂停内容变化重算：out-in 过渡中旧页先离场、新页后入场，
// 等新页真正入场后再统一重查，避免搜到空 DOM 或在过渡中闪烁计数。
let findPendingRouteSwitch = false
let findSwitchFallbackTimer: number | null = null
// 程序化滚动（下一个/上一个、首次命中）期间不要被滚动同步抢走当前命中
let suppressScrollSyncUntil = 0
function refreshFind(keepCurrent = false, silent = false) {
  // 每次查找都确认监听器挂在当前的查找根上。这样无论查找栏是从菜单栏、
  // 工具栏按钮还是 Ctrl+F 唤起，也无论唤起前是否已打开笔记，正文变化都能重算。
  watchFindContent()
  // 保留用户滚动到的相对序号；内容新增/删减后 clamp 到新的总数
  const keep = keepCurrent ? Math.max(0, findCurrent.value - 1) : 0
  suppressFindObserver = true
  suppressScrollSyncUntil = Date.now() + (silent ? 150 : 500)
  try {
    const root = findRoot()
    findRootText = root ? (root.textContent || '') : ''
    // 笔记编辑区用 CSS Highlight（不碰 DOM，打字不扰光标）；其它页用 <mark>
    const r = finder.find(root, findQuery.value, keep, silent, !!selectedNote.value)
    findCount.value = r.count
    findCurrent.value = r.current
    // 短语页用滑动窗口（滚到底追加、滚过阈值裁头），重算后原来的序号可能落到
    // 已滚走的位置 → 用户看不到当前命中（"找不到最后一个 a"）。此时不反向滚动，
    // 而是把当前命中重新锚定到用户正在看的那个匹配项，计数随之更新。
    // 笔记编辑区例外：用户正盯着正文看，重算后要保持原来选中的那一项，
    // 既不反向滚动也不按视口重锚（否则打字时高亮乱跳）。
    if (r.count && !finder.isActiveVisible() && !selectedNote.value) {
      if (keepCurrent) syncFindCurrentToViewport()
      else finder.scrollActiveIntoView('auto')
    }
  } finally {
    // 自己插入/还原 mark 也会触发 MutationObserver，抑制到下一个宏任务再解除
    window.setTimeout(() => { suppressFindObserver = false }, 0)
  }
}
// 正文内容变化（增删词条、展开折叠、分页加载等）后自动重跑查找，
// 保留当前命中的相对位置，只重算总数和它在其中排第几
function watchFindContent() {
  const root = findRoot()
  if (!root) return
  // 根变了（例如先打开笔记、再唤起查找栏）就先断开旧根再重挂，
  // 不能因为「已经有监听器」就直接返回——那会一直盯着旧页面。
  if (findObserver && findObservedRoot === root) return
  if (findObserver) { findObserver.disconnect(); findObserver = null }
  findObservedRoot = root
  findObserver = new MutationObserver(() => {
    if (suppressFindObserver || findPendingRouteSwitch || !findShow.value || !findQuery.value.trim()) return
    // 只是查找自身在插入/还原 <mark>，纯文本没变 → 不算内容变化，跳过
    const root = findRoot()
    if (root && (root.textContent || '') === findRootText) return
    if (findRecalcTimer !== null) clearTimeout(findRecalcTimer)
    findRecalcTimer = window.setTimeout(() => {
      findRecalcTimer = null
      if (!findPendingRouteSwitch && findShow.value && findQuery.value.trim()) refreshFind(true, true)
    }, 120)
  })
  findObserver.observe(root, { childList: true, subtree: true, characterData: true })
}
function stopWatchFindContent() {
  if (findRecalcTimer !== null) { clearTimeout(findRecalcTimer); findRecalcTimer = null }
  if (findObserver) { findObserver.disconnect(); findObserver = null }
  findObservedRoot = null
}
// 查找根随页面切换（列表 ↔ 编辑视图）而变，监听器要跟着重新挂到新根上，
// 否则编辑正文变化时不会重算。
function rebindFindRoot() {
  stopWatchFindContent()
  watchFindContent()
}
// 页面切换动画结束后，用原关键词在新页面重新查找。
// 查找栏始终保持打开——只有用户点关闭 / Esc 才算手动关闭。
function onFindPageEntered() {
  if (findSwitchFallbackTimer !== null) { clearTimeout(findSwitchFallbackTimer); findSwitchFallbackTimer = null }
  if (!findPendingRouteSwitch) return
  findPendingRouteSwitch = false
  if (findShow.value && findQuery.value.trim()) {
    rebindFindRoot()
    refreshFind()
  }
}

// 用户自己滚动时：把"当前命中"更新为滚动容器内最靠近视口上部的可见命中，
// 且只改高亮、不反向滚动；之后的「下一个/上一个」从这个位置继续。
let findScrollTimer: number | null = null
let findScrollRoot: HTMLElement | null = null
function scrollContainerOf(el: HTMLElement, root: HTMLElement): HTMLElement {
  let p = el.parentElement
  while (p && p !== root.parentElement) {
    const cs = getComputedStyle(p)
    if (/(auto|scroll|overlay)/.test(cs.overflowY) || /(auto|scroll|overlay)/.test(cs.overflowX)) return p
    if (p === root) break
    p = p.parentElement
  }
  return root
}
function syncFindCurrentToViewport() {
  const root = findRoot()
  if (!root) return
  // Highlight 模式没有 <mark> 节点，用查找模块提供的命中位置；<mark> 模式仍走 DOM 查询
  const hits = finder.getMode() === 'highlight'
    ? finder.getHitBoxes()
    : Array.from(root.querySelectorAll('mark.pn-find-mark')).map((m) => ({ box: m.getBoundingClientRect(), el: m as HTMLElement }))
  if (!hits.length) return
  let bestIdx = -1
  let bestScore = Infinity
  hits.forEach(({ box, el }, idx) => {
    if (!box.width && !box.height) return
    const cb = scrollContainerOf(el, root).getBoundingClientRect()
    if (box.bottom < cb.top + 2 || box.top > cb.bottom - 2) return
    // 视口上部 1/5 处作为参照，取最接近它的可见命中
    const score = Math.abs((box.top + box.bottom) / 2 - (cb.top + Math.min(80, cb.height * 0.2)))
    if (score < bestScore) { bestScore = score; bestIdx = idx }
  })
  if (bestIdx < 0 || finder.result().current === bestIdx + 1) return
  finder.setActiveSilent(bestIdx)
  findCurrent.value = bestIdx + 1
}
function onFindScroll() {
  if (!findShow.value || !findQuery.value.trim()) return
  if (Date.now() < suppressScrollSyncUntil || findScrollTimer !== null) return
  // 用定时器节流而非 rAF：后台/无头环境里 rAF 可能被挂起，定时器更可靠
  findScrollTimer = window.setTimeout(() => {
    findScrollTimer = null
    if (Date.now() < suppressScrollSyncUntil) return
    syncFindCurrentToViewport()
  }, 50)
}
function onFindInput() { refreshFind() }
// Element Plus 内置清空按钮：清空后清掉高亮/计数并把焦点还给输入框，方便直接输下一个词
function onFindClear() {
  nextTick(() => {
    refreshFind()
    findInputRef.value?.focus?.()
  })
}
// 导航：单次跳转用平滑动画，快速连点由 findInPage 自动降级为即时滚动。
// 平滑滚动期间不要被滚动同步抢走当前命中；动画结束后再兜底检查一次是否真的
// 到达了目标（跨容器/长距离时 smooth 可能被打断），快速连点则立即检查。
let findNavFallbackTimer: number | null = null
function navigateFind(dir: 1 | -1) {
  if (findNavFallbackTimer !== null) { clearTimeout(findNavFallbackTimer); findNavFallbackTimer = null }
  const r = dir > 0 ? finder.next() : finder.prev()
  // ensureFresh 可能因滑动窗口裁剪而重查，计数也要跟着更新
  findCount.value = r.count
  findCurrent.value = r.current
  const smooth = r.behavior === 'smooth'
  suppressScrollSyncUntil = Date.now() + (smooth ? 900 : 400)
  const check = () => {
    findNavFallbackTimer = null
    if (r.count && !finder.isActiveVisible()) finder.scrollActiveIntoView('auto')
  }
  if (smooth) findNavFallbackTimer = window.setTimeout(check, 850)
  else check()
}
function onFindNext() { navigateFind(1) }
function onFindPrev() { navigateFind(-1) }

// 查找栏长按拖动：鼠标左键或手指按住空白处约 160ms 后可拖动；
// 短距离移动视为普通点击，输入框/按钮区域不触发拖动。
const findDragging = ref(false)
const findBarStyle = ref<Record<string, string>>({})
let findDragState: {
  id: number; sx: number; sy: number; left: number; top: number
  bar: HTMLElement; moved: boolean
} | null = null
let findLongPressTimer: number | null = null
function onFindBarPointerDown(e: PointerEvent) {
  if (e.pointerType === 'mouse' && e.button !== 0) return
  const target = e.target as HTMLElement
  if (target.closest('input, button, .el-input__clear, .el-input__icon')) return
  const bar = e.currentTarget as HTMLElement
  const host = bar.parentElement
  if (!host) return
  const rect = bar.getBoundingClientRect()
  const hostRect = host.getBoundingClientRect()
  findDragState = {
    id: e.pointerId, sx: e.clientX, sy: e.clientY,
    left: rect.left - hostRect.left, top: rect.top - hostRect.top,
    bar, moved: false
  }
  if (findLongPressTimer !== null) clearTimeout(findLongPressTimer)
  findLongPressTimer = window.setTimeout(() => {
    const s = findDragState
    if (!s || s.moved) return
    findDragging.value = true
    try { s.bar.setPointerCapture(s.id) } catch { /* 部分环境不支持捕获，忽略 */ }
  }, 160)
}
function onFindBarPointerMove(e: PointerEvent) {
  const s = findDragState
  if (!s) return
  if (!findDragging.value) {
    if (Math.abs(e.clientX - s.sx) > 8 || Math.abs(e.clientY - s.sy) > 8) {
      s.moved = true
      if (findLongPressTimer !== null) { clearTimeout(findLongPressTimer); findLongPressTimer = null }
    }
    return
  }
  const host = s.bar.parentElement
  if (!host) return
  const hostRect = host.getBoundingClientRect()
  const maxLeft = Math.max(4, hostRect.width - s.bar.offsetWidth - 4)
  const maxTop = Math.max(4, window.innerHeight - hostRect.top - s.bar.offsetHeight - 4)
  const left = Math.max(4, Math.min(maxLeft, s.left + e.clientX - s.sx))
  const top = Math.max(4, Math.min(maxTop, s.top + e.clientY - s.sy))
  findBarStyle.value = { left: `${left}px`, top: `${top}px`, right: 'auto' }
}
function onFindBarPointerUp() {
  if (findLongPressTimer !== null) { clearTimeout(findLongPressTimer); findLongPressTimer = null }
  if (findDragState && findDragging.value) {
    try { findDragState.bar.releasePointerCapture(findDragState.id) } catch { /* 忽略 */ }
  }
  findDragState = null
  findDragging.value = false
}
function openFind() {
  findShow.value = true
  findQuery.value = ''
  currentMenu.value = ''
  nextTick(() => {
    findInputRef.value?.focus?.()
  })
}
// 编辑器工具栏的「搜索」按钮通过事件唤起同一个查找栏（wangEditor 菜单实例内无法 inject）
window.addEventListener('pn-open-find', openFind)
function closeFind() {
  findShow.value = false
  findQuery.value = ''
  finder.clear()
  findCount.value = 0
  findCurrent.value = 0
}
// 查找栏打开时拦截 Ctrl/Cmd+F：聚焦搜索框并全选（浏览器原生查找栏在 WebView 里不可程序化调出）
function onGlobalKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
    e.preventDefault()
    if (!findShow.value) openFind()
    else nextTick(() => {
      const input = findInputRef.value?.input
      if (input) { input.focus(); input.select() }
    })
    return
  }
  if (e.key === 'Escape' && findShow.value) {
    e.preventDefault()
    closeFind()
  }
}
// -------------------- 批处理短语（帮助菜单）--------------------
// 全局遮罩浮窗：复制提示词 → 文本框粘贴 → 录入（后端 batch_entry_phrases 单命令单事务合并入库）
const batch_phrase_show = ref(false)
const batch_input = ref('')
// 录入中标志（防重入 + 按钮 loading 态）
const batch_entry_loading = ref(false)
// 录入失败明细（keyword + reason），展示在浮窗内，保留原文便于修正重试
const batch_failures = ref<Array<{ keyword: string; reason: string }>>([])
// 发给 AI 的提示词：约定「每行一条：关键词 | 短语 | 释义」的输出格式，便于后续解析录入
const BATCH_PROMPT = `按照下面固定JSON结构生成英语短语词组，只输出标准合法纯JSON文本，该文本可以直接保存为 .json 文件，不要任何解释、备注、代码块标记、多余文字。
结构说明：
顶层：{"phrases": [数组]}
数组内每一项：
{
  "type": "A~Z" 或者 "#",
  "first_char": "首字符",
  "keyword": "分组标题",
  "list": [{"phrase": "短语", "translate": "释义"}, ...]
}
规则：
1. type = "A~Z"：以英文单词为核心；first_char 是该单词小写首字母；keyword写核心单词，可加简短说明
2. type = "#"：中文语法/同义主题归类；first_char是keyword第一个汉字；keyword写中文主题
3. 同义替换短语使用格式："= xxx"，和示例风格统一
4. 释义暂时不确定可以填空字符串 ""，但是【phrase 绝对不允许为空】，list 里面每一项的 phrase 必须是有效短语
5. list 数组不能为空，每个分组至少包含1条短语
6. 适配中职/初中英语考点，易混搭配放同一组；格式保持和样例一致，sb. sth. 缩写规范
参考样例结构：
{
  "phrases": [
    {
      "type": "A~Z",
      "first_char": "d",
      "keyword": "different",
      "list": [
        { "phrase": "be different from…", "translate": "不同于…" },
        { "phrase": "= be same as…", "translate": "与…一样" }
      ]
    },
    {
      "type": "#",
      "first_char": "留",
      "keyword": "留神，小心",
      "list": [
        { "phrase": "take care", "translate": "" },
        { "phrase": "look out", "translate": "" },
        { "phrase": "watch out", "translate": "" }
      ]
    }
  ]
}`

function openBatchPhrase() {
  batch_input.value = ''
  batch_phrase_show.value = true
}
function closeBatchPhrase() {
  batch_phrase_show.value = false
}
// -------------------- 选取本地 JSON 文件（多选）--------------------
// 隐藏的文件选择框，仅由「选取 JSON 文件」按钮触发
const json_file_input = ref<HTMLInputElement | null>(null)
function pickJsonFiles() {
  json_file_input.value?.click()
}
// 容错解析：与后端同策略（剥 ``` 围栏 → 直接 parse → 失败截取首尾花括号），取出 phrases 数组
function parsePhrasesText(text: string): any[] {
  let s = text.trim()
  if (s.startsWith('```')) {
    const nl = s.indexOf('\n')
    if (nl >= 0) s = s.slice(nl + 1).trim()
    if (s.endsWith('```')) s = s.slice(0, -3).trim()
  }
  let v: any
  try {
    v = JSON.parse(s)
  } catch {
    const a = s.indexOf('{')
    const b = s.lastIndexOf('}')
    if (a < 0 || b <= a) throw new Error('不是有效的 JSON')
    v = JSON.parse(s.slice(a, b + 1))
  }
  const arr = v && typeof v === 'object' ? v.phrases : null
  if (!Array.isArray(arr)) throw new Error('JSON 中缺少 phrases 数组')
  return arr
}
// 统一的「文件数组 → 解析 → 合并进文本框」逻辑：选取文件与拖放共用。
// 逐个解析，成功的短语合并进文本框（已有内容可解析则保留合并，否则覆盖），
// 失败的文件记名跳过——最终都走与粘贴完全相同的「录入」链路
async function loadJsonFiles(files: File[]) {
  if (files.length === 0) return
  let merged: any[] = []
  let groupCount = 0
  const failed: string[] = []
  // 文本框已有内容：能解析出 phrases 就与之合并（不丢用户已粘贴的内容），否则覆盖
  if (batch_input.value.trim()) {
    try {
      merged = parsePhrasesText(batch_input.value)
    } catch {
      /* 旧内容不可解析，直接覆盖 */
    }
  }
  for (const f of files) {
    try {
      const arr = parsePhrasesText(await f.text())
      merged.push(...arr)
      groupCount += arr.length
    } catch {
      failed.push(f.name)
    }
  }
  if (groupCount === 0) {
    ElMessage({
      message: failed.length ? `以下文件解析失败：${failed.join('、')}` : '文件中没有可录入的内容',
      type: 'error',
      offset: 50,
      duration: 4000,
    })
    return
  }
  batch_input.value = JSON.stringify({ phrases: merged }, null, 2)
  batch_failures.value = []
  const tail = failed.length ? `，${failed.length} 个文件解析失败：${failed.join('、')}` : ''
  ElMessage({
    message: `已读入 ${groupCount} 组短语${tail}`,
    type: failed.length ? 'warning' : 'success',
    offset: 50,
    duration: 3000,
  })
}
// 选取文件：读取后交给共用逻辑，并立即重置 input 保证同一文件可再次选取
async function onJsonFileChange(e: Event) {
  const input = e.target as HTMLInputElement
  const files = Array.from(input.files || [])
  input.value = ''
  await loadJsonFiles(files)
}
// -------------------- 拖放识别 JSON 文件（电脑端；移动端无拖放交互，自然不触发）--------------------
// 高亮状态由「心跳」管理，不依赖 dragleave——WebView2 从桌面拖入的瞬间会误发
// relatedTarget=null 的 dragleave，若由它清除高亮会“闪一下就断”。
// 规则：dragover 持续刷新时间戳保持高亮；超过 600ms 没有新 dragover（已离开/松手/取消）才熄灭。
const batch_drag_over = ref(false)
let batch_drag_last = 0
let batch_drag_timer: number | null = null
// 拖动经过：仅当拖的是文件（而非文本/链接）时高亮浮窗
function onBatchDragOver(e: DragEvent) {
  if (e.dataTransfer && Array.from(e.dataTransfer.types || []).includes('Files')) {
    batch_drag_last = Date.now()
    if (!batch_drag_over.value) batch_drag_over.value = true
  }
}
// 松手：取文件列表走与「选取文件」完全相同的解析载入流程，高亮立即熄灭
async function onBatchDrop(e: DragEvent) {
  batch_drag_over.value = false
  const files = Array.from(e.dataTransfer?.files || [])
  if (files.length === 0) return
  await loadJsonFiles(files)
}
// 复制提示词到剪贴板：优先 Clipboard API，失败回退 execCommand（兼容个别 WebView 环境）
async function copyBatchPrompt() {
  let ok = false
  try {
    await navigator.clipboard.writeText(BATCH_PROMPT)
    ok = true
  } catch {
    const ta = document.createElement('textarea')
    ta.value = BATCH_PROMPT
    ta.style.cssText = 'position:fixed;opacity:0;pointer-events:none'
    document.body.appendChild(ta)
    ta.select()
    ok = document.execCommand('copy')
    document.body.removeChild(ta)
  }
  if (ok) {
    ElMessage({ message: '提示词已复制到剪贴板', type: 'success', offset: 50, duration: 2000 })
  } else {
    ElMessage({ message: '复制失败，请手动选择内容复制', type: 'error', offset: 50, duration: 3000 })
  }
}
// 录入：把文本框里的 JSON 文本交给后端解析，按双行两态模型自动合并入库。
// 成功 → 摘要提示 + 清空文本框 + 触发短语页刷新（浮窗保留，可连续录入）；
// 有失败条目 → 不清空原文，失败明细展示在浮窗内，修正后可再次录入。
async function doBatchEntry() {
  if (batch_entry_loading.value) return
  const text = batch_input.value
  if (!text.trim()) {
    ElMessage({ message: '请先粘贴短语内容', type: 'warning', offset: 50, duration: 2000 })
    return
  }
  batch_entry_loading.value = true
  batch_failures.value = []
  try {
    const res: any = await invoke('batch_entry_phrases', { json_text: text })
    batch_failures.value = Array.isArray(res?.failed) ? res.failed : []
    const parts: string[] = []
    if (res?.added) parts.push(`新增 ${res.added} 组`)
    if (res?.appended) parts.push(`追加 ${res.appended} 组`)
    if (res?.merged) parts.push(`合并 ${res.merged} 组`)
    if (res?.skipped_phrases) parts.push(`跳过 ${res.skipped_phrases} 条已存在`)
    if (res?.skipped_groups) parts.push(`${res.skipped_groups} 组已完整存在`)
    if (parts.length === 0) parts.push('没有新内容')
    const summary = parts.join('，')
    if (batch_failures.value.length > 0) {
      ElMessage({
        message: `录入完成，${batch_failures.value.length} 条失败：${summary}`,
        type: 'warning',
        offset: 50,
        duration: 4000,
      })
    } else {
      ElMessage({ message: `录入完成：${summary}`, type: 'success', offset: 50, duration: 3000 })
    }
    // 有实际写入就刷新短语页；全部成功才清空文本框（失败时保留原文便于修正）
    const wrote = (res?.added || 0) + (res?.appended || 0) + (res?.merged || 0) > 0
    if (wrote) phraseDataVersion.value++
    if (batch_failures.value.length === 0) batch_input.value = ''
  } catch (e: any) {
    ElMessage({ message: `录入失败：${e?.message ?? e}`, type: 'error', offset: 50, duration: 4000 })
  } finally {
    batch_entry_loading.value = false
  }
}
// 点击菜单栏以外的区域自动收起
function onDocClick(e: MouseEvent) {
  const el = e.target as HTMLElement | null
  if (el && !el.closest('.menu-bar')) {
    currentMenu.value = ''
  }
}
// Element Plus消息提示、弹窗
import { ElMessage, ElMessageBox } from 'element-plus'
// Vue3
import { ref, onMounted, onUnmounted, computed, nextTick, watch, provide } from 'vue'
// 路由：router 实例在 main.js 中通过 app.use(router) 挂载，路由表见 src/router/index.js
import { useRouter, useRoute } from 'vue-router'
const router = useRouter()
const route = useRoute()
// 侧边导航菜单：数组驱动，新增页面只需往这里加一项（label 显示文案，icon 导航图标，path 路由地址）
// 切换用 router.replace 而非 push，避免每次切换都往浏览器/历史栈里塞一条记录
const navItems = [
  { label: '常规短语', icon: 'ChatDotRound', path: '/phrase' },
  { label: '英语笔记', icon: 'Notebook', path: '/note' },
  { label: '发音规则', icon: 'Headset', path: '/vrules' },
]
// 顶部选项卡容器（手机端横向滚动，桌面端由 CSS 变竖排侧栏）
const optionPanelRef = ref<HTMLElement | null>(null)
// 手机端横向选项卡：激活项贴近/越过容器左右边缘时，把整条选项卡往回平移，
// 让它完整露出来（已完全可见时不动，避免每次点击都把整条轨道居中乱跳）。
// 用 getBoundingClientRect 差值而非 offsetLeft，避免受定位父级影响；
// 桌面竖排容器没有横向溢出（scrollWidth == clientWidth），函数自动跳过。
function ensureActiveTabVisible(smooth = true) {
  const panel = optionPanelRef.value
  if (!panel) return
  if (panel.scrollWidth <= panel.clientWidth + 1) return
  const active = panel.querySelector('li.active') as HTMLElement | null
  if (!active) return
  const panelRect = panel.getBoundingClientRect()
  const activeRect = active.getBoundingClientRect()
  // 边缘安全距离：留一点余量，不让激活项紧贴裁切线
  const edge = Math.min(24, panelRect.width * 0.15)
  let delta = 0
  if (activeRect.left < panelRect.left + edge) {
    delta = activeRect.left - panelRect.left - edge
  } else if (activeRect.right > panelRect.right - edge) {
    delta = activeRect.right - panelRect.right + edge
  }
  if (Math.abs(delta) < 1) return
  const maxScroll = panel.scrollWidth - panel.clientWidth
  const target = Math.max(0, Math.min(maxScroll, panel.scrollLeft + delta))
  if (Math.abs(target - panel.scrollLeft) < 1) return
  panel.scrollTo({ left: target, behavior: smooth ? 'smooth' : 'auto' })
}
function onNavClick(path: string) {
  router.replace(path)
  // 点击当下就触发一次：路由 watch 也会调，但这里保证过渡动画期间不漏掉
  nextTick(() => ensureActiveTabVisible(true))
}
// 路由变化（点击、浏览器前进后退）后，等 active 类更新再滚；冷启动在 onMounted 里单独处理
watch(() => route.path, () => {
  // 切换选项卡不关闭全局搜索：保留关键词与查找栏，等新页入场后重新高亮。
  // 只暂停过渡期间的内容变化重算，避免旧页离场 / 新页未就绪时误刷新。
  if (findShow.value && findQuery.value.trim()) {
    findPendingRouteSwitch = true
    if (findSwitchFallbackTimer !== null) clearTimeout(findSwitchFallbackTimer)
    // 兜底：动画事件因故未触发时也要完成重查
    findSwitchFallbackTimer = window.setTimeout(onFindPageEntered, 700)
  }
  nextTick(() => ensureActiveTabVisible(true))
})
// 打开 / 关闭笔记整屏编辑视图：查找栏保持打开，等浮层入场/离场后用同一关键词
// 重新高亮，且范围只限当前那一页（见 findRoot）。只有用户手动 ✕ / Esc 才关闭。
watch(selectedNote, (note, oldNote) => {
  if (note === oldNote) return
  if (!findShow.value || !findQuery.value.trim()) return
  findPendingRouteSwitch = true
  if (findSwitchFallbackTimer !== null) clearTimeout(findSwitchFallbackTimer)
  findSwitchFallbackTimer = window.setTimeout(onFindPageEntered, 700)
})
// PhrasePage 刷新改用 phraseDataVersion 触发，App 不再持有子组件 ref

// -------------------- 重复导入弹窗状态 --------------------
// 重复关键词冲突弹窗
const dupDialogShow = ref(false)
const dupList = ref<any[]>([])
const dupIndex = ref(0)
const dupRemember = ref(false)
const currentDup = computed(() => dupList.value[dupIndex.value] || null)
// 冲突裁决进行中：单条 invoke 期间 / 后台批处理期间。用于按钮 loading、防重入、入口守卫
const dupResolving = ref(false)
// 当前正在执行的裁决动作，供对应按钮显示 loading（批处理路径弹窗已关，用不到）
const dupResolvingAction = ref<'overwrite' | 'append' | null>(null)

// 逐条（未勾选「直接执行」）乐观推进：点击决定后立即推进弹窗，后端裁决进【串行队列】后台处理。
// 之所以串行：resolve_duplicates_batch 每次都新开 SQLite 连接，前端若瞬间并发多个写入会触发 SQLITE_BUSY 丢失数据。
const dupQueue = ref<any[]>([])
const dupAllDecided = ref(false)
const dupErrCount = ref(0)
const dupCancelled = ref(false)
let dupWorkerRunning = false

function enqueueDup(item: { action: string; keyword: string; new_list: string }) {
  dupQueue.value.push(item)
  if (!dupWorkerRunning) {
    dupWorkerRunning = true
    runDupWorker()
  }
}

// 后台串行消费队列：同一时刻只跑一条 resolve_duplicates_batch，避免并发写同一 SQLite 文件报 SQLITE_BUSY
async function runDupWorker() {
  while ((dupQueue.value.length > 0 || !dupAllDecided.value) && !dupCancelled.value) {
    if (dupQueue.value.length === 0) {
      await new Promise((r) => setTimeout(r, 30))   // 用户还在逐条点击，稍等更多决定入队
      continue
    }
    const item = dupQueue.value.shift()!
    try {
      const res: any = await invoke('resolve_duplicates_batch', {
        action: item.action,
        items: [{ keyword: item.keyword, new_list: item.new_list }],
      })
      const errs = (res?.errors?.length) || 0
      // 追加 / 覆盖成功写入的这一条也算「导入完成」，按 resolved 累加进总数
      importSuccessCount.value += Number(res?.resolved) || 0
      if (errs > 0) console.error('批量处理部分失败', res.errors)
      dupErrCount.value += errs
    } catch (e) {
      console.error('处理失败', e)
      dupErrCount.value += 1
    }
  }
  dupWorkerRunning = false
  const errs = dupErrCount.value
  dupQueue.value = []
  dupAllDecided.value = false
  dupErrCount.value = 0
  const cancelled = dupCancelled.value
  dupCancelled.value = false
  if (cancelled) return                        // 取消时不弹完成提示，由 cancelDupImport 处理
  loading_show.value = false
  loading_text.value = ''
  await finishImport(errs)
}

// -------------------- 设置窗口（外观主题）--------------------
const settings_show = ref(false)
const currentSetting = ref('appearance')
// 第一栏选项：数据驱动，加「版本更新」等只需往这里加一项
const settingsOptions = [
  { key: 'appearance', label: '外观' },
  { key: 'update', label: '版本更新' },
]
const themeOptions = [
  { value: 'system', label: '跟随系统' },
  { value: 'light', label: '浅色模式' },
  { value: 'dark', label: '深色模式' },
]
const theme = ref('system')
// 主题配色：与深浅主题正交的一套主色（极光蓝 / 桃露粉 / 星云紫 / 薄荷绿 / 暮霞橙），由 data-scheme 驱动
const schemeOptions = [
  { value: 'blue', label: '极光蓝' },
  { value: 'sakura', label: '桃露粉' },
  { value: 'nebula', label: '星云紫' },
  { value: 'mint', label: '薄荷绿' },
  { value: 'dusk', label: '暮霞橙' },
]
const scheme = ref('blue')
// 版本更新：当前版本号（tauri.conf.json 的 version）+ 启动时检查更新开关（持久化到 config.json）
const appVersion = ref('')
const checkUpdateOnStart = ref(true)

async function openSettings() {
  // 打开时回填当前已保存的主题与配色，保证选项高亮正确
  try {
    const saved = await invoke('load_app_config')
    theme.value = saved && saved !== '' ? saved : 'system'
  } catch {
    theme.value = 'system'
  }
  try {
    const savedScheme = await invoke('load_app_scheme')
    scheme.value = savedScheme && savedScheme !== '' ? savedScheme : 'blue'
  } catch {
    scheme.value = 'blue'
  }
  try {
    checkUpdateOnStart.value = await invoke('load_check_on_start')
  } catch {
    checkUpdateOnStart.value = true
  }
  currentSetting.value = 'appearance'
  settings_show.value = true
}

function closeSettings() {
  settings_show.value = false
}

// 选择外观主题：即时切换 data-theme（main.js 的 MutationObserver 会自动同步 html.dark），
// 并持久化到 config.json（文件不存在由后端按「跟随系统」兜底）
function selectTheme(t: string) {
  theme.value = t
  if (t === 'system') {
    delete document.documentElement.dataset.theme
  } else {
    document.documentElement.dataset.theme = t
  }
  invoke('save_app_config', { theme: t }).catch((e: any) =>
    console.warn('保存主题失败', e)
  )
}

// 选择主题配色：只改 <html data-scheme>，所有 --pn-* 令牌随之切换，
// 深浅主题不受影响；同样持久化到 config.json（由后端保留其它字段）
function selectScheme(s: string) {
  scheme.value = s
  if (!s || s === 'blue') {
    delete document.documentElement.dataset.scheme
  } else {
    document.documentElement.dataset.scheme = s
  }
  invoke('save_app_scheme', { scheme: s }).catch((e: any) =>
    console.warn('保存配色失败', e)
  )
}

// 保存「启动时检查更新」开关到 config.json（update.check_on_start）
function selectCheckOnStart(v: boolean) {
  checkUpdateOnStart.value = v
  invoke('save_check_on_start', { checkOnStart: v }).catch((e: any) =>
    console.warn('保存检查更新开关失败', e)
  )
}
const importing = ref(false)

// 本次导入真正成功落库的总条数 = 第一阶段 res.imported（短语词条 + 笔记目录/笔记）
// + 冲突弹窗里「追加 / 覆盖」裁决成功的条数（resolve_duplicates_batch 的 res.resolved）。
// 跳过与裁决失败不计入；完成提示统一显示这个总数，不单独显示「重复关键词条数」。
const importSuccessCount = ref(0)

// 全局初始化遮罩：App 首次渲染即为 true（盖住启动到初始化完成之间的空窗），
// 由 onMounted 调 ensure_init 完成后置 false 隐藏。
const init_show = ref(true)

// ====================== 冷更新（启动时后端检测，失败静默）======================
const updateDialogShow = ref(false)
const updateInfo = ref<{ version: string; url: string; notes: string; file_name: string }>({
  version: '', url: '', notes: '', file_name: ''
})
const updateDownloading = ref(false)
const updateProgress = ref(0)
// 安装阶段的两类反馈（都放在弹窗里，不再用 toast——长文案会把一行提示框撑爆）：
//   updateError：真失败（下载失败 / 拉不起安装器），可点「重试」重新下载
//   updateNeedPermission + updatePermissionTip：安装包已下载，只差「安装未知应用」授权，
//     授权后点「继续安装」复用本地安装包（后端 install_downloaded_update），不重新下载
const updateError = ref('')
const updateNeedPermission = ref(false)
const updatePermissionTip = ref('')
let updateProgressUnlisten: (() => void) | null = null

// 每次重新开始（下载 / 继续安装）前先清掉上一条反馈，避免旧文案串场
function resetUpdateFeedback() {
  updateError.value = ''
  updateNeedPermission.value = false
  updatePermissionTip.value = ''
}

// 后端安装结果的统一落地：need_permission = 留在弹窗里给指引，其余（installed / cancelled）关弹窗
function applyInstallResult(res: any) {
  if (res?.status === 'need_permission') {
    updateNeedPermission.value = true
    updatePermissionTip.value =
      res.message ||
      '请先在系统设置中允许本应用「安装未知应用」，然后返回这里点「继续安装」。'
    updateProgress.value = 100
    return
  }
  updateDialogShow.value = false
}

// 后台下载：只关弹窗，下载任务在后端继续跑，进度监听保持到完成
function backgroundDownload() {
  updateDialogShow.value = false
}

// 取消（未开始下载时）：等同关弹窗，不再提示（update.json 的日期后端已记，当天不再提示）
function cancelUpdate() {
  updateDialogShow.value = false
}

// 取消下载（正在下载时）：通知后端中断下载并删除半成品安装包，等同「不更新了」
async function cancelDownload() {
  updateDownloading.value = false
  updateProgress.value = 0
  updateDialogShow.value = false
  try {
    await invoke('cancel_update')
  } catch (e) {
    console.warn('取消下载失败（已忽略）', e)
  }
}

// 弹窗关闭动画结束后复位（下次打开是干净状态）
function onUpdateDialogClosed() {
  updateProgress.value = 0
  resetUpdateFeedback()
}

// 更新：订阅进度事件 → 调后端下载（后端下载完自动拉起安装）
async function startUpdate() {
  updateDownloading.value = true
  updateProgress.value = 0
  resetUpdateFeedback()
  try {
    if (!updateProgressUnlisten) {
      updateProgressUnlisten = await listen('update-progress', (e: any) => {
        const p = e.payload?.percent
        if (typeof p === 'number') updateProgress.value = p
      })
    }
    const res: any = await invoke('download_update', {
      url: updateInfo.value.url,
      fileName: updateInfo.value.file_name
    })
    applyInstallResult(res)
  } catch (e: any) {
    console.error('更新失败', e)
    // 后端的真实失败原因直接显示在弹窗里（原来是固定文案 + 一行 toast）
    const reason = typeof e === 'string' ? e : (e?.message || String(e ?? ''))
    updateError.value = reason || '更新失败，请稍后重试'
    updateProgress.value = 0
  } finally {
    updateDownloading.value = false
  }
}

// 「继续安装」：安装包已下载好，只是缺「安装未知应用」授权。
// 走 install_downloaded_update 复用本地文件，避免授权完回来又白下一遍几十 MB。
async function installPending() {
  updateDownloading.value = true
  updateError.value = ''
  updateProgress.value = 100
  try {
    const res: any = await invoke('install_downloaded_update', {
      fileName: updateInfo.value.file_name
    })
    applyInstallResult(res)
  } catch (e: any) {
    console.error('继续安装失败', e)
    const reason = typeof e === 'string' ? e : (e?.message || String(e ?? ''))
    updateError.value = reason || '安装失败，请稍后重试'
  } finally {
    updateDownloading.value = false
  }
}

// 检查更新：后端全权负责（日期记账 / API / 版本比对 / 失败判定），前端只按 status 展示
// 后端返回 status: available=弹更新框 / up_to_date=已是最新 / error=失败原因 / silent=静默
// force=true（手动点「检查更新」）绕过「当天已查」记账；启动调用不传，仍按每天一次
async function checkForUpdate(force = false) {
  try {
    const res: any = await invoke('check_update', { force })
    if (!res) return
    if (res.status === 'available' && res.info?.version && res.info?.url) {
      updateInfo.value = {
        version: res.info.version || '',
        url: res.info.url || '',
        notes: res.info.notes || '',
        file_name: res.info.file_name || ''
      }
      updateDownloading.value = false
      updateProgress.value = 0
      resetUpdateFeedback()
      updateDialogShow.value = true
    } else if (res.status === 'up_to_date') {
      ElMessage({ message: res.message || '当前版本为最新版本', type: 'info', offset: 50, duration: 2500 })
    } else if (res.status === 'error') {
      ElMessage({ message: res.message || '检查更新失败', type: 'error', offset: 50, duration: 3000 })
    }
    // silent：启动检查的无更新/失败情况，什么都不做
  } catch (e) {
    // 后端异常也只在手动检查时提示，启动检查静默
    if (force) {
      ElMessage({ message: `检查更新失败：${e}`, type: 'error', offset: 50, duration: 3000 })
    } else {
      console.warn('检查更新失败（已忽略）', e)
    }
  }
}

// ======================【八、局域网互传：发送文件面板】======================
const send_file_show = ref(false)
const receive_file_show = ref(false)
const loading_show = ref(false)
const loading_text = ref('')
const deviceList = ref<Array<{ id: string; name: string; ip: string; port: number }>>([])
const selectedDeviceIds = ref<string[]>([])
// 更新接收文件面板
async function updateBroadcastState() {
  if (receive_file_show.value) {
    await invoke('start_broadcast')
  } else {
    await invoke('stop_broadcast')
  }
}
// 打开发送面板
async function openSendPanel() {
  send_file_show.value = true
  deviceList.value = []
  selectedDeviceIds.value = []
  await updateBroadcastState()
  try {
    await invoke('start_scan')
  } catch (err) {
    ElMessage({ message: `扫描启动失败：${err}`, type: 'error', offset: 50, duration: 3000 })
  }
}
// 关闭发送面板
async function closeSendPanel() {
  send_file_show.value = false
  await updateBroadcastState()
  try {
    await invoke('stop_scan')
    await invoke('cancel_send')
  } catch (err) {
    console.warn('停止扫描失败', err)
  }
}
// 检测设备是否已经被选中
function toggleDevice(id: string) {
  // 选中的设备id
  const i = selectedDeviceIds.value.indexOf(id)
  if (i >= 0) selectedDeviceIds.value.splice(i, 1)
  else selectedDeviceIds.value.push(id)
}
// 实现发送
async function doSend() {
  if (dupResolving.value) return           // 冲突批处理期间不触发发送，避免 loading 文案互相覆盖
  if (selectedDeviceIds.value.length === 0) {
    ElMessage({ message: '请至少选择一台设备', type: 'warning', offset: 50, duration: 3000 })
    return
  }
  loading_show.value = true
  loading_text.value = '正在发送文件...'
  try {
    await invoke('send_files_to_devices', { deviceIds: selectedDeviceIds.value })
    ElMessage({ message: '发送完成', type: 'success', offset: 50, duration: 3000 })
    await closeSendPanel()
  } catch (err) {
    if (String(err).includes('取消')) {
      ElMessage({ message: '已取消发送', type: 'info', offset: 50, duration: 2000 })
    } else {
      ElMessage({ message: `发送失败：${err}`, type: 'error', offset: 50, duration: 3000 })
    }
  } finally {
    loading_show.value = false
  }
}

// ======================【九、局域网互传：接收文件面板】======================
// 接收文件列表
const receivedFiles = ref<Array<any>>([])
// 当前在接收面板中选中的发送设备（点击设备=选中，按「导入」按钮后统一导入）
const selectedDevices = ref<string[]>([])
// 已弹过「收到文件」提醒的设备集合：同一设备在一次接收会话中只弹一次，避免每个文件都弹
const notifiedDevices = new Set<string>()
// 当前正在导入的文件 id（用于导入完成后按设备清理源文件）
const currentImportIds = ref<string[]>([])
// 接收定时器变量
let receiveTimer: number | null = null

async function updateReceiving() {
  await invoke('set_receiving', { enabled: receive_file_show.value })
}

async function openReceivePanel() {
  receive_file_show.value = true
  // 新接收会话开始：清空提醒去重集合，下次该设备发来仍会提醒一次
  notifiedDevices.clear()
  await updateBroadcastState()
  await updateReceiving()
  await refreshReceived()
  receiveTimer = window.setInterval(refreshReceived, 1000)
}

async function refreshReceived() {
  try {
    const files = await invoke('get_received_files')
    receivedFiles.value = files as any
  } catch {}
}

async function closeReceivePanel() {
  receive_file_show.value = false
  // 关闭面板：清空提醒去重集合，下次重新打开接收会话可再次提醒
  notifiedDevices.clear()
  await updateBroadcastState()
  await updateReceiving()
  try {
    await invoke('clear_received_files')
    receivedFiles.value = []
  } catch (e) {
    console.warn('清理接收缓存失败', e)
  }
  if (receiveTimer !== null) {
    clearInterval(receiveTimer)
    receiveTimer = null
  }
}

// 按发送设备分组：[{ device, files: [...] }]
const deviceGroups = computed(() => {
  const map = new Map<string, any[]>()
  for (const f of receivedFiles.value) {
    if (!map.has(f.from_device)) map.set(f.from_device, [])
    map.get(f.from_device)!.push(f)
  }
  return Array.from(map.entries()).map(([device, files]) => ({ device, files }))
})

// 点击设备行：仅切换选中态（高亮），不直接导入
function toggleDeviceSelect(device: string) {
  const i = selectedDevices.value.indexOf(device)
  if (i >= 0) {
    selectedDevices.value.splice(i, 1)
  } else {
    selectedDevices.value.push(device)
  }
}

// 仅隐藏接收面板、停止轮询与广播/接收，不删除磁盘上的源文件
// （源文件稍后由 doImport 消费）。供「导入」按钮在导入前先关面板使用。
async function hideReceivePanel() {
  receive_file_show.value = false
  // 导入前软关闭面板：清空提醒去重集合，下次接收会话可再次提醒
  notifiedDevices.clear()
  if (receiveTimer !== null) {
    clearInterval(receiveTimer)
    receiveTimer = null
  }
  // 关闭面板即停止接收，避免面板已隐藏却仍可被远端写入
  await updateBroadcastState()
  await updateReceiving()
}

// 按下「导入」：先关面板（保留磁盘文件），再开始导入选中的设备文件。
// 面板关闭不等待所有文件接收完成，导入在后台（loading 遮罩）进行。
async function onImportButtonClick() {
  if (selectedDevices.value.length === 0) return
  const ids: string[] = []
  for (const grp of deviceGroups.value) {
    if (selectedDevices.value.includes(grp.device)) {
      for (const f of grp.files) ids.push(f.id)
    }
  }
  if (ids.length === 0) return
  // 先关面板（不删磁盘文件），再开始导入
  await hideReceivePanel()
  selectedDevices.value = []
  await doImport(ids)
}

async function doImport(fileIds?: string[]) {
  // 防重入：正在导入（含等待冲突弹窗/后台批处理）时，忽略重复触发，避免同一批文件被导入两次
  if (importing.value || dupResolving.value || dupDialogShow.value) return
  const ids = fileIds && fileIds.length > 0 ? fileIds : currentImportIds.value
  if (ids.length === 0) { return }
  currentImportIds.value = ids
  loading_show.value = true
  loading_text.value = '正在导入数据...'
  importing.value = true
  importSuccessCount.value = 0
  try {
    const res: any = await invoke('import_received_files', {
      fileIds: ids,
    })
    importSuccessCount.value = Number(res.imported) || 0
    const duplicates = res.duplicates || []
    if (duplicates.length > 0) {
      dupList.value = duplicates
      dupIndex.value = 0
      dupRemember.value = false
      dupDialogShow.value = true
    } else {
      ElMessage({ message: `导入完成，共导入 ${importSuccessCount.value} 条`, type: 'success', offset: 50, duration: 3000 })
      await afterImportSuccess(ids)
    }
  } catch (err) {
    if (String(err).includes('取消')) {
      ElMessage({ message: '已取消导入', type: 'info', offset: 50, duration: 2000 })
    } else {
      ElMessage({ message: `导入失败：${err}`, type: 'error', offset: 50, duration: 3000 })
    }
    phraseDataVersion.value++
      noteDataVersion.value++
  } finally {
    loading_show.value = false
    importing.value = false
  }
}

// 导入成功后：刷新数据版本、仅清理本次导入的源文件。
// 注意：面板已在按下「导入」时由 hideReceivePanel 先行关闭，此处不再关面板；
// 未选中的其它设备文件仍保留在磁盘，下次打开面板可继续导入。
async function afterImportSuccess(ids: string[]) {
  phraseDataVersion.value++
  noteDataVersion.value++
  if (ids.length === 0) return            // zip 导入无接收源文件，跳过磁盘清理
  try {
    await invoke('clear_received_files_by_ids', { fileIds: ids })
  } catch (e) {
    console.warn('清理已导入文件失败', e)
  }
  receivedFiles.value = receivedFiles.value.filter((f: any) => !ids.includes(f.id))
}

async function resolveCurrent(action: 'overwrite' | 'skip' | 'append') {
  const dup = currentDup.value
  if (!dup || dupResolving.value) return   // 防重入：连点/双击不重复执行、不双增 dupIndex

  // 复选框含义：勾选 = 对本次剩余冲突全部执行相同操作（批量、最快）；未勾选 = 逐条处理、每条重新确认
  const applyAll = dupRemember.value

  // 跳过：保留原有，无需调后端
  if (action === 'skip') {
    if (applyAll) { await finishImport(); return }   // 全跳过=空操作，直接收尾，不闪遮罩
    if (dupIndex.value + 1 < dupList.value.length) dupIndex.value += 1
    else await finishImport()
    return
  }

  // 勾选「直接执行」：立即关闭弹窗，剩余冲突（含当前条）后台批量处理，复用全局加载遮罩
  if (applyAll) {
    const items = dupList.value.slice(dupIndex.value)
    dupDialogShow.value = false            // 先关弹窗，再出遮罩（遮罩 z-index 低于弹窗，必须避免并存）
    dupRemember.value = false
    dupResolving.value = true
    loading_show.value = true              // importing 保持 false → 遮罩上不出现「取消」按钮
    loading_text.value = `正在导入... (共 ${items.length} 条)`
    let errCount = 0
    try {
      // 一次 invoke 批量处理：后端共用一个 SQLite 连接，避免 N 次 IPC 往返 + N 次 Connection::open
      const res: any = await invoke('resolve_duplicates_batch', {
        action,
        items: items.map((d: any) => ({ keyword: d.keyword, new_list: d.new_list })),
      })
      errCount = (res.errors && res.errors.length) || 0
      // 批量追加 / 覆盖成功的条数一并计入「导入完成」总数
      importSuccessCount.value += Number(res.resolved) || 0
      if (errCount > 0) console.error('批量处理部分失败', res.errors)
    } catch (e) {
      errCount = items.length               // 整体失败：全部计为失败
      console.error('批量处理失败', e)
    } finally {
      loading_show.value = false
      loading_text.value = ''
      dupResolving.value = false
      await finishImport(errCount)          // 统一收尾：toast + 版本刷新 + 源文件清理 + 清残留
    }
    return
  }

  // 未勾选：逐条决定，但点击后【立即推进】（乐观）——弹窗马上跳到下一条冲突（或关闭），
  // 后端裁决进串行队列后台逐条处理，全部点完且队列排空后由 runDupWorker 统一弹“导入完成”。
  enqueueDup({ action, keyword: dup.keyword, new_list: dup.new_list })
  const isLast = dupIndex.value + 1 >= dupList.value.length
  if (isLast) {
    dupAllDecided.value = true
    dupDialogShow.value = false          // 关弹窗，下面用全局遮罩代替（遮罩 z-index 低于弹窗，必须避免并存）
    loading_show.value = true            // importing 保持 false → 遮罩上不出现「取消」按钮
    loading_text.value = '正在导入...'
  } else {
    dupIndex.value += 1                  // 立即进入下一条，弹窗继续显示，不转圈等待
  }
}

async function cancelImport() {
  await invoke('cancel_import')
  loading_show.value = false
  loading_text.value = ''
  phraseDataVersion.value++
      noteDataVersion.value++
}

async function finishImport(errCount = 0) {
  // 完成提示统一显示本次真正落库的总条数：第一阶段静默写入（短语 + 笔记）
  // + 弹窗里追加/覆盖裁决成功的条数；跳过、失败、取消都不计入
  const total = importSuccessCount.value
  dupDialogShow.value = false
  dupRemember.value = false
  dupList.value = []                       // 清冲突列表残留
  dupIndex.value = 0
  if (errCount > 0) {
    ElMessage({
      message: `导入完成，共导入 ${total} 条，${errCount} 条处理失败`,
      type: 'warning', offset: 50, duration: 3000
    })
  } else {
    ElMessage({
      message: `导入完成，共导入 ${total} 条`,
      type: 'success', offset: 50, duration: 3000
    })
  }
  await afterImportSuccess(currentImportIds.value)
}

function cancelDupImport() {
  dupCancelled.value = true               // 通知后台队列停止，已入队未处理的不再写入
  dupQueue.value = []
  dupAllDecided.value = false
  dupDialogShow.value = false
  dupRemember.value = false
  dupList.value = []                       // 清冲突列表残留
  dupIndex.value = 0
  loading_show.value = false
  loading_text.value = ''
  ElMessage({ message: '已取消，部分数据已导入', type: 'info', offset: 50, duration: 3000 })
  phraseDataVersion.value++
      noteDataVersion.value++
  // 已写入的数据保留，清理这些源文件以免再次导入时重复触发冲突
  invoke('clear_received_files_by_ids', { fileIds: currentImportIds.value }).catch(() => {})
  receivedFiles.value = receivedFiles.value.filter((f: any) => !currentImportIds.value.includes(f.id))
}

// ======================【十、本地导入导出zip备份】======================
function todayStr() {
  const d = new Date()
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}${m}${day}`
}
//打开导出对话框
async function openExportDialog() {
  if (dupResolving.value) return           // 冲突批处理期间不触发导出，避免 loading 文案互相覆盖
  try {
    const savePath = await save({
      defaultPath: `phrase_backup_${todayStr()}.zip`,
      filters: [{ name: 'Zip', extensions: ['zip'] }],
    })
    if (!savePath) return
    loading_show.value = true
    loading_text.value = '正在导出...'
    const count = await invoke('export_subject_dirs', { savePath })
    ElMessage({
      message: `导出成功，共 ${count} 个文件，已保存到您选择的位置`,
      type: 'success',
      offset: 50,
      duration: 4000
    })
  } catch (err) {
    ElMessage({
      message: `导出失败：${err}`,
      type: 'error',
      offset: 50,
      duration: 3000
    })
  } finally {
    loading_show.value = false
  }
}
// 打开导入对话框
async function openImportDialog() {
  // 防重入：主导入 / 冲突批处理 / 冲突弹窗期间禁止再次触发导入
  if (importing.value || dupResolving.value || dupDialogShow.value) return
  try {
    // 只支持单文件导入
    const result = await open({
      multiple: false,
      filters: [{ name: 'Zip', extensions: ['zip'] }],
    })
    const zipPath = Array.isArray(result) ? result[0] : result
    if (!zipPath) return
    currentImportIds.value = []            // zip 导入无接收源文件：防止 finishImport/cancelDupImport 用陈旧接收文件 id 误清理
    loading_show.value = true
    loading_text.value = '正在导入...'
    importing.value = true
    importSuccessCount.value = 0
    const res: any = await invoke('import_subject_zip', { zipPath })
    importSuccessCount.value = Number(res.imported) || 0
    // 重复的关键词列表
    const duplicates = res.duplicates || []
    if (duplicates.length > 0) {
      dupList.value = duplicates
      dupIndex.value = 0
      dupRemember.value = false
      dupDialogShow.value = true
    } else {
      ElMessage({
        message: `导入完成，共导入 ${importSuccessCount.value} 条，跳过 ${res.skipped} 个文件`,
        type: 'success',
        offset: 50,
        duration: 3000
      })
      phraseDataVersion.value++
      noteDataVersion.value++
    }
  } catch (err) {
    if (String(err).includes('取消')) {
      ElMessage({ message: '已取消导入', type: 'info', offset: 50, duration: 2000 })
    } else {
      ElMessage({ message: `导入失败：${err}`, type: 'error', offset: 50, duration: 3000 })
    }
    phraseDataVersion.value++
      noteDataVersion.value++
  } finally {
    loading_show.value = false
    importing.value = false
  }
}

// ======================【生命周期】======================
// 用来保存 取消监听的函数,表示“停止监听某个事件的函数”
let unlisteners: UnlistenFn[] = []
// 安卓返回键监听的注销函数（null = 未注册成功）
let backButtonUnlisten: (() => void) | null = null

onMounted(async () => {
  // 冷启动若停在非默认页，把对应选项卡滚进视口（无动画，避免首屏闪动）
  nextTick(() => ensureActiveTabVisible(false))
  // 读取当前版本号供「设置 > 版本更新」展示（失败留空，不影响启动）
  getVersion().then(v => { appVersion.value = v }).catch(() => {})
  // 启动期全局初始化：弹遮罩 → 跑 ensure_init（建 init.json / 基础词迁移）→ 收遮罩。
  // 即便后端异常也确保遮罩关闭、appReady 置位，避免卡死白屏。
  try {
    await invoke('ensure_init')
  } catch (e) {
    console.error('初始化失败', e)
  } finally {
    appReady.value = true
    init_show.value = false
    // 刷新词库 / 笔记（首次安装迁移后才有的数据，确保子页面加载到最新内容）
    phraseDataVersion.value++
    noteDataVersion.value++
  }

  // 冷更新：遵循「启动时检查更新」开关（config.json 的 update.check_on_start）
  // 遮罩收掉后再异步检查，读配置失败按默认开启处理，失败静默、不阻塞启动
  try {
    const enabled = await invoke('load_check_on_start')
    checkUpdateOnStart.value = enabled
    if (enabled) void checkForUpdate()
  } catch {
    checkUpdateOnStart.value = true
    void checkForUpdate()
  }

  // 安卓硬件返回键统一入口（桌面端永不触发，注册无副作用）：
  // 1. 先看「冷更新弹窗」——模态，优先级最高：未开始下载 = 取消本次更新；
  //    正在下载 = 转后台下载（关弹窗，下载继续跑）。
  // 2. 再走「关闭栈」——关掉最上面一个打开中的界面（互传发送/接收、设置、批处理短语，
  //    以及 PhrasePage 内的添加/修改表单，后开先关）；
  // 3. 栈空则回退 WebView 历史——笔记编辑器/管理模式各自压过历史记录，
  //    由 main.js 的 popstate 消费并关闭（沿用原有机制，行为不变）；
  // 4. 没有历史可退 → 退出应用，与系统默认行为一致。
  try {
    const backListener = await onBackButtonPress(async ({ canGoBack }) => {
      // 冷更新弹窗：按下载状态分派
      if (updateDialogShow.value) {
        if (updateDownloading.value) {
          backgroundDownload() // 正在下载 → 转后台，下载继续
        } else {
          cancelUpdate() // 还没下载 → 取消本次更新
        }
        return
      }
      if (consumeBackPress()) return
      if (canGoBack) {
        window.history.back()
      } else {
        try {
          await exit(0)
        } catch (e) {
          console.warn('退出应用失败', e)
        }
      }
    })
    backButtonUnlisten = () => {
      void backListener.unregister()
    }
  } catch (e) {
    console.warn('注册安卓返回键监听失败', e)
  }

  // 可关闭界面入栈（后开先关）。短语添加/修改面板在 PhrasePage 内自行绑定。
  bindBackClosable(send_file_show, () => {
    void closeSendPanel()
  })
  bindBackClosable(receive_file_show, () => {
    void closeReceivePanel()
  })
  bindBackClosable(settings_show, closeSettings)
  bindBackClosable(batch_phrase_show, closeBatchPhrase)

  // 点击菜单栏外部时收起原生菜单
  document.addEventListener('click', onDocClick)
  // 全局搜索快捷键：Ctrl/Cmd+F 聚焦查找栏、Esc 关闭
  document.addEventListener('keydown', onGlobalKeydown)
  // 正文变化时自动刷新搜索计数/高亮
  watchFindContent()
  // 用户滚动正文时同步"当前命中"（捕获阶段监听所有内部滚动容器）
  findScrollRoot = findRoot()
  if (findScrollRoot) findScrollRoot.addEventListener('scroll', onFindScroll, true)
  // 拖放高亮心跳：每 250ms 检查一次，距最后一次 dragover 超过 600ms 即熄灭
  // （离开窗口 / 松手 / 取消拖放都会停止 dragover，由它兜底收尾，不依赖 dragleave）
  batch_drag_timer = window.setInterval(() => {
    if (batch_drag_over.value && Date.now() - batch_drag_last > 600) {
      batch_drag_over.value = false
    }
  }, 250)
  unlisteners.push(
    await listen('file_received', (e: any) => {
      const dev = e.payload?.from_device || '未知设备'
      console.log('file_received', e.payload)
      // 同一设备只弹一次提醒，避免 phrase_db.db / note_db.db 各弹一次
      if (!notifiedDevices.has(dev)) {
        notifiedDevices.add(dev)
        ElMessage({ message: `接收到来自 ${dev} 的文件`, type: 'info', offset: 50, duration: 2000 })
      }
    })
  )
  unlisteners.push(
    await listen('devices_update', (e: any) => {
      deviceList.value = e.payload || []
    })
  )
})

// 组件销毁时执行（App根组件：程序退出/页面刷新才触发）
onUnmounted(() => {
  // 1. 全部取消Tauri事件监听，清理事件订阅
  unlisteners.forEach(fn => fn())
  // 1b. 取消更新进度监听
  if (updateProgressUnlisten) updateProgressUnlisten()
  // 2. 注销安卓返回键监听，恢复系统默认行为
  if (backButtonUnlisten) backButtonUnlisten()
  // 3. 兜底：强制关闭接收文件的轮询定时器，防止后台死循环
  if (receiveTimer !== null) clearInterval(receiveTimer)
  // 4. 拖放高亮心跳定时器
  if (batch_drag_timer !== null) clearInterval(batch_drag_timer)
  // 5. 移除菜单外部点击监听
  document.removeEventListener('click', onDocClick)
  // 6. 移除全局搜索键盘监听，断开正文变化监听，并清理可能残留的高亮
  document.removeEventListener('keydown', onGlobalKeydown)
  window.removeEventListener('pn-open-find', openFind)
  if (findScrollRoot) findScrollRoot.removeEventListener('scroll', onFindScroll, true)
  if (findScrollTimer !== null) { clearTimeout(findScrollTimer); findScrollTimer = null }
  if (findSwitchFallbackTimer !== null) { clearTimeout(findSwitchFallbackTimer); findSwitchFallbackTimer = null }
  if (findNavFallbackTimer !== null) { clearTimeout(findNavFallbackTimer); findNavFallbackTimer = null }
  stopWatchFindContent()
  finder.clear()
})

</script>

<style scoped>
/* 这里保留：main‑board、option‑panel、transmission‑panel、dup‑mask、loading‑mask 相关样式 */
.rf-device {
  display: block;
  color: var(--pn-text-emphasis, #c9a227);
  font-weight: 600;
}
.rf-path {
  display: block;
  font-size: 0.8rem;
  color: var(--pn-text-sub, #888);
  word-break: break-all;
}
</style>

<style>
:root {
  font-family: var(--pn-font-ui);
  font-size: 1rem;
  line-height: 1.5rem;
  font-weight: 400;
  /* 根文字 / 背景引用 public/index.css 的主题变量（深色自动切换） */
  color: var(--pn-text-app);
  background-color: var(--pn-bg-app);
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
  -webkit-user-select: none;
  -moz-user-select: none;
  -ms-user-select: none;
  user-select: none;
}
html,body {
  overflow: hidden;
  overscroll-behavior: none;
}
ul li,ol li {
  list-style: none;
}
button {
  border: 0;
  all: unset;
}
label {
  color: var(--pn-text-app) !important;
}
a:hover {
  color: var(--pn-primary-deep);
}

/* 冷更新提示弹窗（el-dialog teleport 到 body，故写在全局段） */
.pn-update-dialog {
  /* 手机窄屏：宽度自适应，不超出视口；内容区允许纵向滚动 */
  max-width: calc(100vw - 32px);
  margin-top: 12vh;
}
.pn-update-dialog .el-dialog__header {
  padding-right: calc(var(--el-dialog-padding-primary) + var(--el-message-close-size, 16px));
}
.pn-update-dialog .el-dialog__header .el-dialog__title {
  font-weight: 600;
}
/* 关闭按钮：触控友好（≥44px），主色描边态 */
.pn-update-dialog .el-dialog__headerbtn {
  width: 44px;
  height: 44px;
  top: 2px;
  right: 2px;
  border-radius: var(--pn-radius-full);
}
.pn-update-dialog .el-dialog__headerbtn .el-dialog__close {
  font-size: 18px;
  color: var(--pn-text-sub, #888);
}
.pn-update-dialog .el-dialog__headerbtn:hover .el-dialog__close,
.pn-update-dialog .el-dialog__headerbtn:focus-visible .el-dialog__close {
  color: var(--pn-primary-deep, #409eff);
}
.pn-update-info {
  padding: 4px 0 8px;
}
.pn-update-version {
  font-size: 1rem;
  font-weight: 600;
  color: var(--pn-primary-deep, #409eff);
  margin-bottom: 10px;
}
.pn-update-notes {
  font-size: 0.9rem;
  line-height: 1.6;
  color: var(--pn-text-sub, #888);
  white-space: pre-wrap;
  /* 小屏时限制高度，超出可滚动，避免把弹窗顶出屏幕 */
  max-height: min(180px, 32vh);
  overflow-y: auto;
  overflow-wrap: anywhere;
  -webkit-user-select: text;
  user-select: text;
}
/* 安装阶段提示块：默认走主题色（授权指引），--error 为失败红；
   长文案正常折行、内部可滚动，绝不顶出弹窗——原来这类原因只塞进一行 toast，
   超长英文（Java 异常串、安装包路径）会把提示框撑爆 */
.pn-update-alert {
  margin-top: var(--pn-space-3, 12px);
  padding: 10px 12px;
  max-height: min(180px, 28vh);
  overflow-y: auto;
  border-radius: var(--pn-radius-md, 8px);
  border: 1px solid var(--pn-border-soft, rgba(127, 127, 127, 0.3));
  background-color: var(--pn-bg-inset, rgba(127, 127, 127, 0.08));
  color: var(--pn-text-app);
  font-size: 0.86rem;
  line-height: 1.6;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  -webkit-user-select: text;
  user-select: text;
}
.pn-update-alert--error {
  border-color: var(--pn-danger, #d63b5c);
  color: var(--pn-danger, #d63b5c);
}
.pn-update-progress {
  padding: 8px 0 4px;
}
.pn-update-progress-label {
  font-size: 0.9rem;
  color: var(--pn-text-app);
  margin-bottom: 12px;
}
/* 下载中：「取消下载」在左，「后台下载」在右 */
.pn-update-progress-actions {
  display: flex;
  gap: 10px;
  margin-top: var(--pn-space-4, 16px);
}
.pn-update-progress-actions .el-button {
  flex: 1 1 0;
  min-width: 0;
  margin-left: 0;
}
.pn-update-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}
/* 手机窄屏：底部按钮等宽铺满，触控区域不小于 44px */
@media screen and (max-width: 480px) {
  .pn-update-dialog {
    margin-top: 8vh;
    --el-dialog-padding-primary: 16px;
  }
  .pn-update-footer .el-button {
    flex: 1 1 0;
    min-width: 0;
    margin-left: 0;
  }
}

/* ============ 右侧竖三点「更多」菜单 ============ */
.menu-more { margin-left: auto; }
.menu-more-btn { padding: var(--pn-space-1) var(--pn-space-2); }
/* 竖向三点（⋮）：CSS 绘制，颜色跟随当前文字色，避免用横排的 MoreFilled */
.menu-more-dots {
  display: inline-flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 3px;
  width: 20px;
  height: 20px;
}
.menu-more-dots i {
  display: block;
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background-color: currentColor;
}
.menu-more-list { right: 0; left: auto; }

/* ============ 全局搜索栏（应用内查找，对齐 Ctrl+F）============ */
.pn-find-bar {
  position: absolute;
  top: calc(100% + 6px);
  right: var(--pn-space-3);
  /* 高于笔记整屏编辑视图（z-index:1000），打开文件后查找栏依然可见可操作 */
  z-index: 1100;
  display: flex;
  align-items: center;
  gap: 6px;
  width: min(420px, calc(100vw - 2 * var(--pn-space-3)));
  padding: 8px 10px;
  /* 触摸长按拖动时阻止页面滚动抢走指针事件 */
  touch-action: none;
  border: 1px solid var(--pn-border);
  border-radius: var(--pn-radius-lg);
  background-color: var(--pn-bg-elevated, var(--pn-bg-card));
  box-shadow: var(--pn-shadow-lg, 0 12px 32px rgba(0, 0, 0, .18));
}
.pn-find-input { flex: 1 1 auto; min-width: 0; }
.pn-find-count {
  flex: 0 0 auto;
  min-width: 52px;
  text-align: center;
  font-size: var(--pn-text-sm);
  color: var(--pn-text-sub);
  font-variant-numeric: tabular-nums;
}
.pn-find-nav,
.pn-find-close {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: none;
  border-radius: var(--pn-radius-sm);
  background: transparent;
  color: var(--pn-text-sub);
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
  transition: background-color var(--pn-duration-fast) var(--pn-ease-standard);
}
/* 长按拖动中的视觉反馈 */
.pn-find-bar.is-dragging {
  cursor: grabbing;
  user-select: none;
  box-shadow: 0 12px 32px rgba(0, 0, 0, .32);
  border-color: var(--pn-primary);
}
.pn-find-nav:hover:not(:disabled),
.pn-find-close:hover { background-color: var(--pn-primary-soft); color: var(--pn-primary-deep); }
.pn-find-nav:disabled { opacity: .4; cursor: default; }
.pn-find-close { font-size: 15px; }
.pn-find-enter-active, .pn-find-leave-active { transition: opacity .15s, transform .15s; }
.pn-find-enter-from, .pn-find-leave-to { opacity: 0; transform: translateY(-6px); }

/* 查找命中高亮：mark 由 JS 插入到各子页面 DOM，这段位于非 scoped 的 <style> 块，
   直接写全局选择器即可（切勿用 :deep()，未 scoped 时它不会被编译、规则会被浏览器丢弃） */
/* 普通命中：主题色浅底，保证所有匹配一眼可见 */
mark.pn-find-mark {
  background-color: var(--pn-primary-soft-strongest, rgba(31, 166, 255, .24));
  color: var(--pn-text);
  border-radius: 3px;
  padding: 0 1px;
}
/* 当前命中：深底反白 + 加粗 + 外圈高亮环，明显区别于其它命中 */
mark.pn-find-mark-active {
  position: relative;
  z-index: 1;
  background-color: var(--pn-primary-strong, #0A6DAF);
  color: #fff;
  font-weight: 700;
  border-radius: 4px;
  box-shadow: 0 0 0 2px var(--pn-primary, #1FA6FF), 0 2px 8px rgba(0, 0, 0, .28);
}

/* CSS Custom Highlight：笔记编辑区专用（浏览器绘制、不碰 DOM，打字不扰光标）。
   ::highlight() 只支持 color / background-color / text-decoration / text-shadow，
   不支持 box-shadow / font-weight，故"当前命中"改为更深的实底反白来突出。 */
::highlight(pn-find) {
  background-color: var(--pn-primary-soft-strongest, rgba(31, 166, 255, .24));
  color: var(--pn-text, inherit);
}
::highlight(pn-find-active) {
  background-color: var(--pn-primary-strong, #0A6DAF);
  color: #fff;
}
</style>
