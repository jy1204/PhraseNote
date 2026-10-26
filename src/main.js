// 自托管 Web 字体（Quicksand 拉丁圆体 + ZCOOL KuaiLe 中文显示体，仅用于应用自有文案）
import '@fontsource/quicksand/400.css'
import '@fontsource/quicksand/500.css'
import '@fontsource/quicksand/600.css'
import '@fontsource/quicksand/700.css'
import '@fontsource/zcool-kuaile/400.css'

import { createApp } from "vue";
import App from "./App.vue";
import { invoke } from '@tauri-apps/api/core';
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import './assets/css/public/index.css';
// Element Plus 皮肤须在 EP 样式与自家令牌之后引入（级联顺序即覆写顺序）
import './assets/css/element-plus-theme.css';
import router from './assets/js/router.js'
import { platform } from '@tauri-apps/plugin-os';
import { currentPlatformRef, selectedNote, backNote, manageMode, exitManage } from './assets/js/index.js'
import * as ElementPlusIconsVue from '@element-plus/icons-vue'

const currentPlatform = await platform();
currentPlatformRef.value = currentPlatform
// Android 返回 "android"，Windows 返回 "windows"
// （平台视觉差异已统一由响应式 CSS 表达，此处仅保留平台检测供 JS 行为使用）

// 拦截安卓硬件返回键：编辑器打开时，系统返回键会先回退 WebView 历史，
// 命中下面的 popstate —— 此时我们只关掉编辑器（或回到上一条笔记），而不是退出应用。
// 列表页没有我们压入的历史记录，popstate 不会触发，返回键仍由系统默认处理（退出应用）。
window.addEventListener('popstate', () => {
  if (selectedNote.value) {
    // 编辑器打开：返回键关掉编辑器（或回到上一条笔记）
    backNote();
  } else if (manageMode.value) {
    // 管理模式：返回键直接取消管理模式（浏览器已自动消费 enterManage 压入的历史，无需再 back）
    exitManage();
  }
});

const app = createApp(App)

app.use(router)
app.use(ElementPlus)
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}
// —— 主题同步 ——
// Element Plus 暗色主题依赖 <html class="dark"> 才激活；本项目用 data-theme
// （或系统偏好）控制主题，这里把两者对齐，保证 el-input / el-select / el-button
// 等 Element Plus 组件与自家 --pn-* 深浅色一起切换。
function syncDarkClass() {
  const root = document.documentElement
  const theme = root.dataset.theme
  let dark
  if (theme === 'dark') dark = true
  else if (theme === 'light') dark = false
  else dark = window.matchMedia('(prefers-color-scheme: dark)').matches
  root.classList.toggle('dark', dark)
}
// 启动时应用已保存的外观主题（config.json）；文件不存在则保持「跟随系统」默认
try {
  const savedTheme = await invoke('load_app_config')
  if (savedTheme && savedTheme !== 'system') {
    document.documentElement.dataset.theme = savedTheme
  }
} catch (e) {
  console.warn('读取外观配置失败', e)
}
// 启动时应用已保存的主题配色（config.json）；「科技蓝」是默认值，无需写属性
try {
  const savedScheme = await invoke('load_app_scheme')
  if (savedScheme && savedScheme !== '' && savedScheme !== 'blue') {
    document.documentElement.dataset.scheme = savedScheme
  }
} catch (e) {
  console.warn('读取配色配置失败', e)
}
syncDarkClass()
// 系统深浅偏好变化时同步
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', syncDarkClass)
// 将来若加手动切换按钮（改 data-theme）也能自动同步 dark class
new MutationObserver(syncDarkClass).observe(document.documentElement, {
  attributes: true,
  attributeFilter: ['data-theme'],
})

app.mount('#app')