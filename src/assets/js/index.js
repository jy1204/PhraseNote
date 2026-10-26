import { ref } from "vue";
export const currentMenu = ref('');
export const add_content_panel_show = ref(false);
// 当前平台（android / windows），由 main.js 启动时写入，供 PhrasePage 按需加载平台样式
export const currentPlatformRef = ref('');
// 数据版本号：导入/导出成功后自增，PhrasePage 监听后刷新词库（替代 App 直接持有子组件 ref）
export const phraseDataVersion = ref(0);
// 笔记数据版本号：导入（zip 备份）成功后自增，NotePage 监听后重新加载目录树
// （与 phraseDataVersion 解耦：导入只改对应库，互不影响刷新）
export const noteDataVersion = ref(0);
// 当前正在编辑的笔记（对象或 null）。非 null 时整个 App 切换到全屏编辑视图，隐藏菜单栏与侧边导航
export const selectedNote = ref(null);
// 笔记打开历史栈：打开新笔记时把上一个压栈，返回时弹栈回到上一个（栈空则关闭编辑视图）
export const noteHistory = ref([]);
// 打开某条笔记（压入历史），关闭/清空时重置历史
export function openNote(note) {
  const wasOpen = !!selectedNote.value;
  if (selectedNote.value) noteHistory.value.push(selectedNote.value);
  selectedNote.value = note;
  // 从「列表」首次进入编辑器时压一条历史记录：
  // 安卓硬件返回键会优先回退 WebView 历史，从而触发 popstate 关掉编辑器，
  // 而不是直接 finish 掉 Activity（退到桌面）。已在编辑器中切换笔记时不重复压。
  if (!wasOpen && typeof window !== 'undefined' && window.history) {
    window.history.pushState({ pnEditor: true }, '');
  }
}
// 返回：若有历史则回到上一条，否则关闭编辑视图
export function backNote() {
  if (noteHistory.value.length > 0) {
    selectedNote.value = noteHistory.value.pop();
  } else {
    selectedNote.value = null;
  }
}
// 清空编辑状态（回到列表并重置历史）
export function closeNote() {
  selectedNote.value = null;
  noteHistory.value = [];
}

// 管理模式（批量移动 / 删除）：是否处于管理模式 + 勾选状态（提升为共享，供 main.js 返回键拦截使用）
export const manageMode = ref(false);
export const checkedFolders = ref(new Set());
export const checkedNotes = ref(new Set());
// 进入管理模式：压一条历史记录，使安卓硬件返回键能关掉管理模式（而非退出应用）
export function enterManage() {
  if (manageMode.value) return;
  manageMode.value = true;
  if (typeof window !== 'undefined' && window.history) {
    window.history.pushState({ pnManage: true }, '');
  }
}
// 退出管理模式（返回键 / 取消按钮共用）：仅重置状态，不处理 history。
// history 的平衡交给调用方：返回键由浏览器 popstate 自动消费；取消按钮需手动 history.back()。
export function exitManage() {
  manageMode.value = false;
  checkedFolders.value.clear();
  checkedNotes.value.clear();
}