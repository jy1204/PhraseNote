import { createApp } from "vue";
import App from "./App.vue";
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import ('./assets/css/public/index.css');
import './assets/css/public/reka_styles.css'
import { platform } from '@tauri-apps/plugin-os';
import * as ElementPlusIconsVue from '@element-plus/icons-vue'

const currentPlatform = await platform(); 
// Android 返回 "android"，Windows 返回 "windows"
if (currentPlatform === 'android') {
    import ('./assets/css/mobile/index.css');
} else if (currentPlatform === 'windows') {
    import ('./assets/css/desktop/index.css');
}

const app = createApp(App)

app.use(ElementPlus)
for (const [key, component] of Object.entries(ElementPlusIconsVue)) {
  app.component(key, component)
}
app.mount('#app')