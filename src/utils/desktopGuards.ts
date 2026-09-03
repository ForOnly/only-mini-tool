/**
 * 桌面守卫：始终拦截浏览器默认右键与整页拖入。
 * 工具自有右键应 stopPropagation，避免触发全局监听；并自行 preventDefault。
 */
export function installDesktopGuards(): void {
  window.addEventListener(
    "contextmenu",
    (event) => {
      event.preventDefault();
    },
    false,
  );

  window.addEventListener("dragover", (event) => {
    event.preventDefault();
  });
  window.addEventListener("drop", (event) => {
    event.preventDefault();
  });
}
