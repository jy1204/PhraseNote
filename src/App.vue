<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { currentMenu, add_content_panel_show } from "./assets/js/index"
import {
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarPortal,
  MenubarRoot,
  MenubarTrigger,
} from 'reka-ui'
import { ElMessage, ElMessageBox } from 'element-plus'
import { reactive, ref, onMounted, nextTick, computed, toRaw } from 'vue'
import { Search, CircleCloseFilled, Edit, DeleteFilled } from '@element-plus/icons-vue'
import { onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { save, open } from '@tauri-apps/plugin-dialog'

// ============ 表单状态 ============

// 表单模式：'add' 添加 | 'edit' 编辑
const formMode = ref('add')

// 编辑时记录原始 keyboard（用于更新时定位）
const editingKeyboard = ref('')

const typeList = ref([
  '*','A','B','C','D','E','F','G','H','I','J','K','L','M',
  'N','O','P','Q','R','S','T','U','V','W','X','Y','Z','#'
])

// 当前激活项，默认选中 *
const activeChar = ref('*')

// 添加/编辑共用的表单对象
const form = reactive({
  type: 'A~Z',
  first_char: '',
  keyboard: '',
  list: [{ phrase: '', translate: '' }]
})

// 折叠面板展开状态
const activeNames = ref(['1'])

// 新建表单的快照：关闭添加面板时保存，下次打开恢复
const savedAddState = ref<null | {
  type: string
  keyboard: string
  first_char: string
  list: { phrase: string; translate: string }[]
  activeNames: string[]
}>(null)

// 查重弹窗相关
const dupDialogShow = ref(false)
const dupList = ref<any[]>([])
const dupIndex = ref(0)
const dupRemember = ref(false)
const currentDup = computed(() => dupList.value[dupIndex.value] || null)
const importing = ref(false)

// ============ 表单快照 ============

// 把当前 form 保存为「新建表单快照」
function snapshotAddForm() {
  savedAddState.value = {
    type: form.type,
    keyboard: form.keyboard,
    first_char: form.first_char,
    list: form.list.map(p => ({ ...p })),
    activeNames: [...activeNames.value],
  }
}

// 打开添加表单
async function openAddForm() {
  formMode.value = 'add'
  editingKeyboard.value = ''
  add_content_panel_show.value = true

  if (savedAddState.value) {
    // 恢复上次保存的新建内容
    form.type = savedAddState.value.type
    form.keyboard = savedAddState.value.keyboard
    form.first_char = savedAddState.value.first_char
    form.list = savedAddState.value.list.map(p => ({ ...p }))
    activeNames.value = [...savedAddState.value.activeNames]
  } else {
    // 首次打开，用默认值
    form.type = 'A~Z'
    form.keyboard = ''
    form.first_char = ''
    form.list = [{ phrase: '', translate: '' }]
    activeNames.value = ['1']
  }

  await nextTick()
  keyboardInput.value?.focus()
}

// 关闭添加面板
function closeAddPanel() {
  if (formMode.value === 'add') {
    snapshotAddForm()
  }
  add_content_panel_show.value = false
}

// 打开编辑表单
function openEditForm(item) {
  // 防御：若当前是添加模式，先把新建内容存起来，避免被编辑数据覆盖
  if (formMode.value === 'add') {
    snapshotAddForm()
  }

  formMode.value = 'edit'
  editingKeyboard.value = item.keyboard
  form.type = item.type
  form.keyboard = item.keyboard
  form.first_char = item.first_char
  form.list = item.list.map(p => ({ ...p }))
  activeNames.value = ['1']
  add_content_panel_show.value = true
}

// 编辑入口
function edit_current_item(item) {
  openEditForm(item)
}

// ============ 主列表 ============

const allPhraseList = ref([])
const show_data = ref(true)
const search_keyboard_input = ref(null)
const search_keyboard_input_content = ref('')

function clear_search_input() {
  search_keyboard_input_content.value = ''
  search_keyboard_input.value?.focus()
}

async function loadDataFromDb() {
  try {
    const res = await invoke("load_all_phrases")
    allPhraseList.value = res.map(item => {
      let list = []
      try {
        list = JSON.parse(item.list)
      } catch (e) {
        console.error('list 解析失败', item, e)
      }
      return {
        id: item.id,
        type: item.type,
        first_char: item.first_char,
        keyboard: item.keyboard,
        list: JSON.parse(item.list)
      }
    })
    show_data.value = allPhraseList.value.length === 0
    console.log("加载完成", allPhraseList.value)
  } catch (err) {
    console.error("启动加载数据库失败", err)
    ElMessage({
      message: `加载数据失败：${err}`,
      type: 'error',
      offset: 50,
      duration: 3000,
      customClass: 'no-hover-pause'
    })
  }
}

// ============ 提交保存 ============

const onSubmit = async () => {
  form.keyboard = form.keyboard.trim().toLowerCase()
  form.first_char = form.keyboard[0]
  form.list.forEach(item => {
    item.phrase = item.phrase.trim()
    item.translate = item.translate.trim()
  })

  // 关键词不能为空
  if (!form.keyboard) {
    ElMessage({ message: '关键词不能为空', type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
    return
  }

  // 首字符校验
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

  // 短语不能为空
  if (form.list.length === 0 || form.list.some(item => !item.phrase)) {
    ElMessage({ message: '短语内容不能为空', type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
    return
  }

  try {
    const raw = toRaw(form)
    if (formMode.value === 'edit') {
      await invoke('delete_phrase', { keyboard: editingKeyboard.value })
    }
    await invoke('save_phrase', {
      type: raw.type,
      first_char: raw.first_char,
      keyboard: raw.keyboard,
      list: JSON.stringify(raw.list),
    })
    ElMessage({
      message: formMode.value === 'edit' ? '修改成功' : '保存成功',
      type: 'success', offset: 50, duration: 3000, customClass: 'no-hover-pause'
    })
    await loadDataFromDb()

    if (formMode.value === 'edit') {
      // 编辑提交：关闭面板，切回 add 模式，不动快照
      add_content_panel_show.value = false
      formMode.value = 'add'
      editingKeyboard.value = ''
    } else {
      // 添加提交成功：清空表单内容（类型保留），并同步更新快照
      form.keyboard = ''
      form.first_char = ''
      form.list = [{ phrase: '', translate: '' }]
      activeNames.value = ['1']

      savedAddState.value = {
        type: form.type,
        keyboard: '',
        first_char: '',
        list: [{ phrase: '', translate: '' }],
        activeNames: ['1'],
      }

      await nextTick()
      keyboardInput.value?.focus()
    }
  } catch (err) {
    console.error('保存失败', err)
    ElMessage({ message: `保存失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
  }
}

// ============ 删除 ============

async function deletePhrase(item) {
  console.log('删除', item.keyboard)
  try {
    await ElMessageBox.confirm(`确定删除「${item.keyboard}」吗？`, '提示', {
      confirmButtonText: '确定',
      cancelButtonText: '取消',
      type: 'warning',
    })
    await invoke('delete_phrase', { keyboard: item.keyboard })
    ElMessage({ message: '删除成功', type: 'success', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
    await loadDataFromDb()
  } catch (err) {
    if (err === 'cancel') return
    console.error('删除失败', err)
    ElMessage({ message: `删除失败：${err}`, type: 'error', offset: 50, duration: 3000, customClass: 'no-hover-pause' })
  }
}

// ============ 短语列表操作 ============

const keyboardInput = ref(null)

function clear_keyboard_content() {
  form.keyboard = ''
  keyboardInput.value?.focus()
}

function removeItem(index) {
  form.list.splice(index, 1)
  activeNames.value = activeNames.value.filter(
    name => Number(name) <= form.list.length
  )
}

function allDelete() {
  form.list = []
  activeNames.value = []
  appendItem()
}

const formScroll = ref(null)

function scrollToBottom() {
  const el = formScroll.value
  if (el) {
    el.scrollTo({
      top: el.scrollHeight,
      behavior: 'smooth'
    })
  }
}

async function appendItem() {
  form.list.push({ phrase: '', translate: '' })
  activeNames.value.push(String(form.list.length))
  await nextTick()
  scrollToBottom()
}

const handleChange = (val) => {
  console.log(val)
}

// ============ 主列表筛选 ============

const filteredPhraseList = computed(() => {
  const keyword = search_keyboard_input_content.value.trim().toLowerCase()
  if (keyword) {
    return allPhraseList.value.filter(item =>
      item.keyboard.toLowerCase().includes(keyword)
    )
  }

  const char = activeChar.value
  if (char === '*') {
    return allPhraseList.value
  }
  if (char === '#') {
    return allPhraseList.value.filter(item =>
      /^[\u4e00-\u9fa5]/.test(item.keyboard)
    )
  }
  return allPhraseList.value.filter(item =>
    item.keyboard[0]?.toLowerCase() === char.toLowerCase()
  )
})

function handleCharClick(char) {
  activeChar.value = char
  search_keyboard_input_content.value = ''
}

// ============ 发送相关 ============

const send_file_show = ref(false)
const receive_file_show = ref(false)
const loading_show = ref(false)
const loading_text = ref('')

const deviceList = ref<Array<{id:string;name:string;ip:string;port:number}>>([])
const selectedDeviceIds = ref<string[]>([])
const sendProgress = ref({ current: 0, total: 0, device: '', file: '' })

async function updateBroadcastState() {
  if (receive_file_show.value) {
    await invoke('start_broadcast')
  } else {
    await invoke('stop_broadcast')
  }
}

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

function toggleDevice(id: string) {
  const i = selectedDeviceIds.value.indexOf(id)
  if (i >= 0) selectedDeviceIds.value.splice(i, 1)
  else selectedDeviceIds.value.push(id)
}

async function doSend() {
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

// ============ 接收相关 ============

const receivedFiles = ref<Array<any>>([])
const selectedFileIds = ref<string[]>([])
let receiveTimer: number | null = null

async function updateReceiving() {
  await invoke('set_receiving', { enabled: receive_file_show.value })
}

async function openReceivePanel() {
  receive_file_show.value = true
  selectedFileIds.value = []
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

function toggleFile(id: string) {
  const i = selectedFileIds.value.indexOf(id)
  if (i >= 0) selectedFileIds.value.splice(i, 1)
  else selectedFileIds.value.push(id)
}

async function doImport() {
  if (selectedFileIds.value.length === 0) { return }

  loading_show.value = true
  loading_text.value = '正在导入数据...'
  importing.value = true

  try {
    const res: any = await invoke('import_received_files', {
      fileIds: selectedFileIds.value,
    })
    const duplicates = res.duplicates || []
    if (duplicates.length > 0) {
      dupList.value = duplicates
      dupIndex.value = 0
      dupRemember.value = false
      dupDialogShow.value = true
    } else {
      ElMessage({ message: `导入完成，新增 ${res.imported} 条`, type: 'success', offset: 50, duration: 3000 })
      await loadDataFromDb()
      await invoke('clear_received_files')
      receivedFiles.value = []
      closeReceivePanel()
    }
  } catch (err) {
    if (String(err).includes('取消')) {
      ElMessage({ message: '已取消导入', type: 'info', offset: 50, duration: 2000 })
    } else {
      ElMessage({ message: `导入失败：${err}`, type: 'error', offset: 50, duration: 3000 })
    }
    await loadDataFromDb()
  } finally {
    loading_show.value = false
    importing.value = false
  }
}

async function resolveCurrent(action: 'overwrite' | 'skip') {
  const dup = currentDup.value
  if (!dup) return

  if (action === 'overwrite') {
    await invoke('resolve_duplicate_overwrite', {
      keyword: dup.keyword,
      new_list: dup.new_list,
    })
  }

  if (dupRemember.value) {
    const rest = dupList.value.slice(dupIndex.value + 1)
    for (const d of rest) {
      if (action === 'overwrite') {
        try {
          await invoke('resolve_duplicate_overwrite', {
            keyword: d.keyword,      // ← 改成 d
            new_list: d.new_list,    // ← 改成 d
          })
        } catch (e) {
          console.error('覆盖失败', e)
          ElMessage({ message: `覆盖失败：${e}`, type: 'error', offset: 50, duration: 3000 })
        }
      }
    }
    await finishImport()
    return
  }

  if (dupIndex.value + 1 < dupList.value.length) {
    dupIndex.value += 1
  } else {
    await finishImport()
  }
}

async function cancelImport() {
  await invoke('cancel_import')
  loading_show.value = false
  loading_text.value = ''
  await loadDataFromDb()
}

// ============ 事件监听 ============

let unlisteners: UnlistenFn[] = []

onMounted(async () => {
  await loadDataFromDb()

  unlisteners.push(
    await listen('send_progress', (e: any) => {
      sendProgress.value = e.payload
    })
  )
  unlisteners.push(
    await listen('file_received', (e: any) => {
      ElMessage({ message: `收到来自 ${e.payload.from_device} 的文件`, type: 'info', offset: 50, duration: 2000 })
    })
  )
  unlisteners.push(
    await listen('devices_update', (e: any) => {
      deviceList.value = e.payload || []
    })
  )
  console.log(allPhraseList.value.length)
})

async function finishImport() {
  dupDialogShow.value = false
  dupRemember.value = false
  ElMessage({ message: '导入完成', type: 'success', offset: 50, duration: 3000 })
  await loadDataFromDb()
  await invoke('clear_received_files')
  receivedFiles.value = []
  await closeReceivePanel()
}

// ============ 导入/导出 ============

function todayStr() {
  const d = new Date()
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}${m}${day}`
}

async function openExportDialog() {
  try {
    const savePath = await save({
      defaultPath: `phrase_backup_${todayStr()}.zip`,
      filters: [{ name: 'Zip', extensions: ['zip'] }],
    })
    if (!savePath) return  // 用户取消

    loading_show.value = true
    loading_text.value = '正在导出...'

    const count = await invoke('export_subject_dirs', { savePath })
    ElMessage({
      message: `导出成功，共 ${count} 个文件，已保存到您选择的位置`,
      type: 'success', offset: 50, duration: 4000
    })
  } catch (err) {
    ElMessage({ message: `导出失败：${err}`, type: 'error', offset: 50, duration: 3000 })
  } finally {
    loading_show.value = false
  }
}

async function openImportDialog() {
  try {
    const result = await open({
      multiple: false,
      filters: [{ name: 'Zip', extensions: ['zip'] }],
    })
    const zipPath = Array.isArray(result) ? result[0] : result
    if (!zipPath) return

    loading_show.value = true
    loading_text.value = '正在导入...'
    importing.value = true

    const res: any = await invoke('import_subject_zip', { zipPath })
    const duplicates = res.duplicates || []

    if (duplicates.length > 0) {
      dupList.value = duplicates
      dupIndex.value = 0
      dupRemember.value = false
      dupDialogShow.value = true
    } else {
      ElMessage({
        message: `导入完成，新增 ${res.imported} 条，跳过 ${res.skipped} 个文件`,
        type: 'success', offset: 50, duration: 3000
      })
      await loadDataFromDb()
    }
  } catch (err) {
    if (String(err).includes('取消')) {
      ElMessage({ message: '已取消导入', type: 'info', offset: 50, duration: 2000 })
    } else {
      ElMessage({ message: `导入失败：${err}`, type: 'error', offset: 50, duration: 3000 })
    }
    await loadDataFromDb()
  } finally {
    loading_show.value = false
    importing.value = false
  }
}

function cancelDupImport() {
  dupDialogShow.value = false
  dupRemember.value = false
  ElMessage({ message: '已取消，部分数据已导入', type: 'info', offset: 50, duration: 3000 })
  loadDataFromDb()
}

onUnmounted(() => {
  unlisteners.forEach(fn => fn())
  if (receiveTimer !== null) clearInterval(receiveTimer)
})
</script>

<template>
  <main class="container">
    <MenubarRoot v-model="currentMenu" class="menu-bar">
      <MenubarMenu>
        <MenubarTrigger class="menu-options">文件</MenubarTrigger>
          <MenubarPortal>
            <MenubarContent class="menu-list" align="start" :side-offset="5" :align-offset="-3">
              <MenubarItem class="menu-list-item" @click="openImportDialog">导入</MenubarItem>
              <MenubarItem class="menu-list-item" @click="openExportDialog">导出</MenubarItem>
            </MenubarContent>
          </MenubarPortal>
      </MenubarMenu>
      <MenubarMenu>
        <MenubarTrigger class="menu-options">互传</MenubarTrigger>
        <MenubarPortal>
          <MenubarContent class="menu-list" align="start" :side-offset="5" :align-offset="-3">
            <MenubarItem class="menu-list-item" @click="openSendPanel">发送</MenubarItem>
            <MenubarItem class="menu-list-item" @click="openReceivePanel">接收</MenubarItem>
          </MenubarContent>
        </MenubarPortal>
      </MenubarMenu>
      <!-- <MenubarMenu>
        <MenubarTrigger class="menu-options">帮助</MenubarTrigger>
        <MenubarPortal>
          <MenubarContent class="menu-list" align="left" :side-offset="5" :align-offset="-3">
            <MenubarItem class="menu-list-item">Undo</MenubarItem>
          </MenubarContent>
        </MenubarPortal>
      </MenubarMenu> -->
    </MenubarRoot>
    <div id="send-file" v-show="send_file_show">
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
        <div v-if="sendProgress.total > 0" class="send-progress">
          正在发送：{{ sendProgress.current }} / {{ sendProgress.total }} —— {{ sendProgress.file }}
        </div>
        <p>请确保目标连接的是同一个Wi-Fi网络</p>
        <div class="transmission-actions">
          <el-button type="primary" @click="doSend">发送到选中设备</el-button>
        </div>
      </div>
    </div>

    <!-- 接收面板 -->
    <div id="receive-file" v-show="receive_file_show">
      <div class="transmission-panel">
        <button id="close-receive-file" @click="closeReceivePanel">×</button>
        <dl>
          <dt><h2>收到的文件：</h2></dt>
          <dd v-for="f in receivedFiles" :key="f.id"
              :class="{ selected: selectedFileIds.includes(f.id) }"
              @click="toggleFile(f.id)">
            <span>{{ f.relative_path || f.saved_path }}</span>
          </dd>
          <dd v-if="receivedFiles.length === 0" class="empty-dev">暂无收到的文件</dd>
        </dl>
        <p>请确保目标连接的是同一个Wi-Fi网络</p>
        <div class="transmission-actions">
          <el-button type="primary" @click="doImport">导入选中的文件</el-button>
        </div>
      </div>
    </div>

    <!-- 全屏加载遮罩 -->
    <div v-if="loading_show" class="loading-mask">
      <div class="loading-spinner"></div>
      <p>{{ loading_text }}</p>
      <el-button v-if="importing" type="danger" @click="cancelImport" style="margin-top:16px">取消</el-button>
    </div>
    <div class="main-board">
      <div class="option-panel">
        <ul>
          <li class="active">常规短语</li>
          <!-- <li>相关短语</li> -->
        </ul>
      </div>
      <div class="content-panel">
        <div class="type">
          <ol>
            <li v-for="item in typeList" :key="item" :class="{ active: activeChar === item }" @click="handleCharClick(item)">{{ item }}</li>
          </ol>
        </div>
        <!-- <div id="empty-box" style="display: none"> -->
        <!-- 空盒子 -->
        <div id="empty-box" v-show="show_data">
          <!-- <img src="./assets/img/content-empty.webp" alt=""> -->
           <el-empty description="这里空空如也，快去添加新知识吧" />
        </div>
        <div class="content-knowledge" v-show="!show_data">
          <!-- 搜索框 -->
          <div class="search-keyboard">
            <el-input ref="search_keyboard_input" v-model="search_keyboard_input_content" class="seach-input" placeholder="搜索关键词" :prefix-icon="Search" />
            <button type="button" class="search-input-clear" @click="clear_search_input"><el-icon><CircleCloseFilled /></el-icon></button>
          </div>
          <div class="grammar">
            <div v-for="item in filteredPhraseList" :key="item.id" class="knowledge">
              <h1>{{ item.keyboard }}<div class="knowledge-actions"><el-icon @click="deletePhrase(item)"><DeleteFilled /></el-icon><el-icon @click="edit_current_item(item)"><Edit /></el-icon></div></h1>
              <p v-for="(phrase, i) in item.list" :key="i">
                <span>{{ phrase.phrase }}</span><br />
                <span>{{ phrase.translate }}</span>
              </p>
            </div>
          </div>
        </div>
        <div class="add-content-panel" v-show="add_content_panel_show">
          <div class="panel">
            <button id="close-panel" @click="closeAddPanel">×</button>
            <!-- 添加数据用的表单 -->
            <el-form :model="form" label-width="auto">
              <el-form-item label="类型：" style="width: 200px;">
                <el-select v-model="form.type">
                  <el-option label="A~Z" value="A~Z" />
                  <el-option label="#" value="#" />
                </el-select>
              </el-form-item>
              <el-form-item label="关键词:" class="keyboard-box">
                <el-input ref="keyboardInput" v-model="form.keyboard" class="keyboard-input" />
                <button type="button" class="keyboard-clear" @click="clear_keyboard_content"><el-icon><CircleCloseFilled /></el-icon></button>
              </el-form-item>
              <el-form-item>
                <el-card style="width: 100%" shadow="never">
                  <template #header>
                    <div class="card-header">短语：</div>
                    <button type="button" class="all-delete" @click="allDelete">全部清空</button>
                    <el-icon @click="appendItem"><CirclePlus /></el-icon>
                    
                  </template>
                  <div class="demo-collapse" ref="formScroll">
                    <el-collapse v-model="activeNames" @change="handleChange" expand-icon-position="left">
                      <div v-for="(item, index) in form.list" :key="index" class="phrase-content-box">
                        <el-icon class="delete" @click="removeItem(index)">
                          <Delete />
                        </el-icon>
                        <el-collapse-item :title="'#' + (index + 1)" :name="String(index + 1)">
                          <el-input v-model="item.phrase" placeholder="请输入短语" />
                          <el-input v-model="item.translate" placeholder="请输入译文" />
                        </el-collapse-item>
                      </div>
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
      </div>
      <button id="add-knowledge" @click="openAddForm">+</button>
    </div>
  <div v-if="dupDialogShow" class="dup-mask">
    <div class="dup-dialog">
        <h2>发现重复关键词</h2>
        <p class="dup-kw">关键词：<b>{{ currentDup?.keyword }}</b></p>
        <div class="dup-section">
          <div class="dup-label">原有内容：</div>
          <pre>{{ currentDup?.old_list }}</pre>
        </div>
        <div class="dup-section">
          <div class="dup-label">新内容：</div>
          <pre>{{ currentDup?.new_list }}</pre>
        </div>
        <el-checkbox v-model="dupRemember">下次遇到此情况直接执行</el-checkbox>
        <div class="dup-actions">
          <el-button @click="resolveCurrent('skip')">跳过</el-button>
          <el-button type="primary" @click="resolveCurrent('overwrite')">覆盖</el-button>
          <el-button @click="cancelDupImport">取消导入</el-button>
        </div>
      </div>
  </div>
  </main>
</template>


<style scoped>
</style>
<style>
:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 1rem;
  line-height: 1.5rem;
  font-weight: 400;
  color: #0f0f0f;
  background-color: #f6f6f6;
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
  color: #0f0f0f!important;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  label {
    color: #f6f6f6 !important;
  }
}

</style>