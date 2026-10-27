import { contextBridge, ipcRenderer } from 'electron'

/**
 * 注入 `window.clubOA`：服务器配置 / 本机通知 / 平台标识。
 * 见需求 13.2（Electron 壳）与 13.1.1（服务器地址可配置）。
 */
contextBridge.exposeInMainWorld('clubOA', {
  platform: 'electron',
  vendor: 'desktop',
  getServer: () => ipcRenderer.invoke('server:current'),
  setServer: (baseUrl: string) => ipcRenderer.invoke('server:save', baseUrl),
  clearServer: () => ipcRenderer.invoke('server:clear'),
  validate: (baseUrl: string) => ipcRenderer.invoke('server:validate', baseUrl),
  notify: (title: string, body: string) => ipcRenderer.invoke('notify', title, body),
  setUnreadCount: (count: number) => ipcRenderer.invoke('badge:set', count)
})
