export async function currentWebviewWindow() {
  const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
  return getCurrentWebviewWindow();
}
