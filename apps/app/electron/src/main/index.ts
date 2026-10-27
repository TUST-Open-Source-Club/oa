import { app, BrowserWindow, Menu, Notification, Tray, nativeImage } from 'electron'
import { autoUpdater } from 'electron-updater'
import { join } from 'node:path'
import { clearServer, loadConfig, saveServer, validateServer } from './server-config'

let mainWindow: BrowserWindow | null = null
let tray: Tray | null = null

/** 单实例 + 深链（cluboa:// 或 ?server= 参数）。 */
if (!app.requestSingleInstanceLock()) {
  app.quit()
}

app.on('second-instance', (_event, argv) => {
  const server = extractServerArg(argv)
  if (server) {
    void configureServer(server)
  }
  if (mainWindow) {
    if (mainWindow.isMinimized()) mainWindow.restore()
    mainWindow.focus()
  }
})

app.on('open-url', (event, url) => {
  event.preventDefault()
  const server = extractServerArg([url])
  if (server) void configureServer(server)
})

/** 从命令行/URL 提取 server 参数。 */
function extractServerArg(args: string[]): string | null {
  for (const arg of args) {
    const match = arg.match(/[?&]server=([^&]+)/)
    if (match) {
      return decodeURIComponent(match[1])
    }
  }
  return null
}

/** 校验并保存服务器地址，成功后加载门户。 */
async function configureServer(baseUrl: string): Promise<void> {
  const issuer = await validateServer(baseUrl)
  await saveServer(baseUrl, issuer)
  await loadPortal()
}

/** 打开服务器设置页（本地页面）。 */
async function showSetup(message = ''): Promise<void> {
  if (!mainWindow) createWindow()
  if (mainWindow) {
    await mainWindow.loadFile(join(__dirname, '../renderer/setup.html'), { query: { message } })
  }
}

/** 加载已配置的 Web 门户。 */
async function loadPortal(): Promise<void> {
  const config = await loadConfig()
  if (!config.baseUrl) {
    await showSetup()
    return
  }
  if (!mainWindow) createWindow()
  void mainWindow?.loadURL(`${config.baseUrl}/`)
}

/** 创建主窗口（Chromium 内嵌，屏幕共享/会议全平台可用）。 */
function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 960,
    minHeight: 640,
    autoHideMenuBar: true,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      spellcheck: false
    }
  })
  mainWindow.on('closed', () => {
    mainWindow = null
  })
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    void import('electron').then(({ shell }) => shell.openExternal(url))
    return { action: 'deny' }
  })
}

/** 同步未读角标：托盘提示 + Dock/任务栏（macOS/Linux）。 */
function updateBadge(count: number): void {
  const tip = count > 0 ? `社团 OA（${count} 条未读）` : '社团 OA'
  tray?.setToolTip(tip)
  if (process.platform === 'darwin') {
    tray?.setTitle(count > 0 ? String(count) : '')
    app.badgeCount = count
  } else if (process.platform === 'linux') {
    app.badgeCount = count
  }
}

/** 托盘：打开/切换服务器/退出 + 未读角标。 */
function createTray(): void {
  const icon = nativeImage.createEmpty()
  tray = new Tray(icon)
  tray.setToolTip('社团 OA')
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: '打开', click: () => mainWindow?.show() },
      { label: '切换服务器', click: () => void showSetup() },
      { type: 'separator' },
      { label: '退出', click: () => app.quit() }
    ])
  )
}

// 主进程 API：设置页调用
import { ipcMain } from 'electron'
ipcMain.handle('server:validate', async (_event, baseUrl: string) => validateServer(baseUrl))
ipcMain.handle('server:save', async (_event, baseUrl: string) => {
  await configureServer(baseUrl)
})
ipcMain.handle('server:current', async () => loadConfig())
ipcMain.handle('server:clear', async () => {
  await clearServer()
  await showSetup()
})
ipcMain.handle('notify', async (_event, title: string, body: string) => {
  if (Notification.isSupported()) {
    new Notification({ title, body }).show()
  }
})
ipcMain.handle('badge:set', (_event, count: number) => {
  updateBadge(Math.max(0, Math.trunc(count)))
})

app.whenReady().then(async () => {
  createWindow()
  createTray()
  await loadPortal()
  autoUpdater.checkForUpdatesAndNotify().catch(() => undefined)
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow()
    void loadPortal()
  }
})
