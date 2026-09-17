import { isTauri } from "./backend";

/**
 * Toggle fullscreen on the desktop app. The window has no OS title bar, so
 * this is the only way to leave fullscreen (F11 or Cmd/Ctrl+Shift+F).
 * No-op in the browser.
 */
export async function toggleFullscreen(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  await win.setFullscreen(!(await win.isFullscreen()));
}
