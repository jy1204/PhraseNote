import { createRouter, createWebHashHistory } from 'vue-router'

// Tauri 桌面 / 移动端没有服务端路由，使用 hash 模式最稳妥
const routes = [
  {
    path: '/',
    redirect: '/phrase',
  },
  {
    path: '/phrase',
    name: 'phrase',
    // 懒加载：只有路由到 PhrasePage 时才加载组件及其 CSS
    component: () => import('../views/PhrasePage.vue'),
  },
  {
    path: '/note',
    name: 'note',
    // 懒加载：只有路由到 NotePage 时才加载该组件
    component: () => import('../views/NotePage.vue'),
  },
  {
    path: '/vrules',
    name: 'vrules',
    // 发音规则页：懒加载，内容待实现（先占位空壳）
    component: () => import('../views/VRulesPage.vue'),
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

export default router
