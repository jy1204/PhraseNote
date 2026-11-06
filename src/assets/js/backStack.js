// ============================================================
// backStack.js —— 安卓硬件返回键的「可关闭界面」栈
// 用法：把界面可见性 ref 和它的关闭函数绑起来（bindBackClosable），
// 界面打开即入栈、关闭即出栈；App.vue 统一监听返回键事件，
// 每次按下关掉「最上面一个」打开中的界面（后开先关），
// 全部关完后返回 false，由调用方走系统默认行为（回退历史 / 退出应用）。
// ============================================================
import { watch } from 'vue'

// 栈元素：{ id, active, close }
const stack = []
let seq = 0

// 注册一个关闭处理器，返回句柄 id
function registerBackHandler(active, close) {
  const id = ++seq
  stack.push({ id, active, close })
  return id
}

// 注销关闭处理器
function unregisterBackHandler(id) {
  const i = stack.findIndex((h) => h.id === id)
  if (i >= 0) stack.splice(i, 1)
}

// 绑定「可见即可关」的界面：
//   visible —— 界面可见性 ref（true = 打开中）
//   close   —— 该界面「取消 / 关闭」按钮的同一个函数（返回键行为与点按钮完全一致）
// visible 为 true 期间注册，变 false 时自动注销；返回解绑函数（组件销毁时调用）。
export function bindBackClosable(visible, close) {
  let id = null
  const stop = watch(
    visible,
    (v) => {
      if (v) {
        if (id === null) id = registerBackHandler(() => visible.value, close)
      } else if (id !== null) {
        unregisterBackHandler(id)
        id = null
      }
    },
    { immediate: true }
  )
  return () => {
    stop()
    if (id !== null) {
      unregisterBackHandler(id)
      id = null
    }
  }
}

// 返回键按下时调用：关掉最上面一个打开中的界面。
// 返回 true  = 已消费（界面被关闭，调用方无需再做其他处理）；
// 返回 false = 没有可关的界面，调用方走默认行为。
export function consumeBackPress() {
  for (let i = stack.length - 1; i >= 0; i--) {
    if (stack[i].active()) {
      stack[i].close()
      return true
    }
  }
  return false
}
