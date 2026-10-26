<template>
  <main class="container">
    <!-- 顶部菜单栏：文件｜互传（原生实现，替代 Reka‑UI，功能/样式保持一致） -->
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

    <!-- 主业务外壳 -->
    <div class="main-board">
      <div class="option-panel">
        <ul>
          <li
            v-for="item in navItems"
            :key="item.path"
            :class="{ active: route.path === item.path }"
            @click="router.replace(item.path)"
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
        <transition name="pn-page" mode="out-in" appear>
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
          </div>
        </div>
      </div>
    </div>
    </transition>

    <!-- 整屏编辑视图：选中笔记时以 position:fixed 全屏覆盖整个 app（含菜单栏与侧边导航），点返回即关闭；
         开合动画：上滑淡入 / 下滑淡出（覆盖层为 fixed 全屏，无需 out-in） -->
    <transition name="pn-editor" appear>
      <NoteEditor v-if="selectedNote" />
    </transition>
  </main>
</template>

<script setup lang="ts">
/// <reference types="vite/client" />
/// <reference types="element-plus/global" />
// Tauri 核心调用、事件监听API
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
// Tauri 文件对话框：保存、打开文件
import { save, open } from '@tauri-apps/plugin-dialog'
// 外部状态
import { currentMenu, phraseDataVersion, noteDataVersion, selectedNote } from "./assets/js/index.js"
// 全屏笔记编辑视图：选中文件时整屏覆盖原 app（菜单栏 + 侧边导航）
import NoteEditor from "./assets/views/NoteEditor.vue"

// -------------------- 顶部菜单栏（原生实现，替代 Reka‑UI）--------------------
// currentMenu 来自外部状态：'' 表示全部收起，'file' / 'transfer' 表示展开对应菜单
function toggleMenu(name: string) {
  currentMenu.value = currentMenu.value === name ? '' : name
}
// 点击菜单项：先执行对应功能，再收起菜单
function onMenuItem(_name: string, fn: () => void) {
  fn()
  currentMenu.value = ''
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
import { ref, onMounted, onUnmounted, computed } from 'vue'
// 路由：router 实例在 main.js 中通过 app.use(router) 挂载，路由表见 src/router/index.js
import { useRouter, useRoute } from 'vue-router'
const router = useRouter()
const route = useRoute()
// 侧边导航菜单：数组驱动，新增页面只需往这里加一项（label 显示文案，icon 导航图标，path 路由地址）
// 切换用 router.replace 而非 push，避免每次切换都往浏览器/历史栈里塞一条记录
const navItems = [
  { label: '常规短语', icon: 'ChatDotRound', path: '/phrase' },
  { label: '英语笔记', icon: 'Notebook', path: '/note' },
]
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

// -------------------- 设置窗口（外观主题）--------------------
const settings_show = ref(false)
const currentSetting = ref('appearance')
// 第一栏选项：数据驱动，先只放「外观」，以后加「通用 / 关于」等只需往这里加一项
const settingsOptions = [
  { key: 'appearance', label: '外观' },
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
const importing = ref(false)

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
  try {
    const res: any = await invoke('import_received_files', {
      fileIds: ids,
    })
    const duplicates = res.duplicates || []
    if (duplicates.length > 0) {
      dupList.value = duplicates
      dupIndex.value = 0
      dupRemember.value = false
      dupDialogShow.value = true
    } else {
      ElMessage({ message: `导入完成，新增 ${res.imported} 条`, type: 'success', offset: 50, duration: 3000 })
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

  // 未勾选：只处理【当前这一条】，处理完自动跳到下一条冲突继续让你确认（逐条、不一把梭）
  dupResolving.value = true
  dupResolvingAction.value = action
  const single = [{ keyword: dup.keyword, new_list: dup.new_list }]
  try {
    const res: any = await invoke('resolve_duplicates_batch', { action, items: single })
    const errCount = (res.errors && res.errors.length) || 0
    if (errCount > 0) console.error('批量处理部分失败', res.errors)
    if (dupIndex.value + 1 < dupList.value.length) dupIndex.value += 1   // 进入下一条，弹窗继续显示
    else await finishImport(errCount)
  } catch (e) {
    ElMessage({ message: `处理失败：${e}`, type: 'error', offset: 50, duration: 3000 })
    phraseDataVersion.value++
    noteDataVersion.value++
  } finally {
    dupResolving.value = false
    dupResolvingAction.value = null
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
  dupDialogShow.value = false
  dupRemember.value = false
  dupList.value = []                       // 清冲突列表残留
  dupIndex.value = 0
  if (errCount > 0) {
    ElMessage({ message: `导入完成，${errCount} 条处理失败`, type: 'warning', offset: 50, duration: 3000 })
  } else {
    ElMessage({ message: '导入完成', type: 'success', offset: 50, duration: 3000 })
  }
  await afterImportSuccess(currentImportIds.value)
}

function cancelDupImport() {
  dupDialogShow.value = false
  dupRemember.value = false
  dupList.value = []                       // 清冲突列表残留
  dupIndex.value = 0
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
    const res: any = await invoke('import_subject_zip', { zipPath })
    // 重复的关键词列表
    const duplicates = res.duplicates || []
    if (duplicates.length > 0) {
      dupList.value = duplicates
      dupIndex.value = 0
      dupRemember.value = false
      dupDialogShow.value = true
    } else {
      ElMessage({
        message: `导入完成，新增 ${res.imported} 条，跳过 ${res.skipped} 个文件`,
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

onMounted(async () => {
  // 点击菜单栏外部时收起原生菜单
  document.addEventListener('click', onDocClick)
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
  // 2. 兜底：强制关闭接收文件的轮询定时器，防止后台死循环
  if (receiveTimer !== null) clearInterval(receiveTimer)
  // 3. 移除菜单外部点击监听
  document.removeEventListener('click', onDocClick)
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
</style>
