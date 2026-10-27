import { computed } from 'vue'

/**
 * 客户端壳环境检测与桥接（见需求 13.1.1、AND-010、HARM-009、DESK-014）：
 * - Android/HarmonyOS：`window.ClubOA`（JS Bridge）
 * - Electron：`window.clubOA`（contextBridge）
 * - iOS PWA / 浏览器：无桥，纯 Web
 */

/** 移动壳桥接口。 */
export interface ClubOABridge {
  /** 平台标识：android / harmony。 */
  platform?: () => string
  /** 推送厂商。 */
  vendor?: () => string
  /** 厂商推送 token（可为同步/异步）。 */
  pushToken?: () => string | null | Promise<string | null>
  /** 清除服务器配置并回到设置页。 */
  switchServer?: () => void
}

/** Electron 桥接口。 */
export interface ElectronBridge {
  platform: string
  vendor: string
  clearServer: () => Promise<void>
  notify: (title: string, body: string) => Promise<void>
  /** 同步未读到托盘/Dock 角标。 */
  setUnreadCount?: (count: number) => Promise<void>
}

declare global {
  interface Window {
    ClubOA?: ClubOABridge
    clubOA?: ElectronBridge
  }
}

/** 运行平台。 */
export type ShellPlatform = 'android' | 'harmony' | 'electron' | 'ios-pwa' | 'browser'

/** 是否 iOS 主屏 PWA。 */
function detectIOSPWA(win: Window, nav: Navigator): boolean {
  const standalone = (nav as unknown as { standalone?: boolean }).standalone === true
  const mobile = /iPad|iPhone|iPod/.test(nav.userAgent)
  return mobile && standalone
}

/** 客户端壳相关能力。 */
export function useShell() {
  const win = typeof window === 'undefined' ? undefined : window
  const nav = typeof navigator === 'undefined' ? undefined : navigator

  /** Electron 壳。 */
  const isElectron = computed(() => win?.clubOA?.platform === 'electron')
  /** 移动壳（Android/HarmonyOS）。 */
  const isMobileShell = computed(() => !!win?.ClubOA && !isElectron.value)
  /** iOS 主屏 PWA。 */
  const isIOSPWA = computed(() => !!win && !!nav && detectIOSPWA(win, nav))

  /** 运行平台。 */
  const platform = computed<ShellPlatform>(() => {
    if (isElectron.value) return 'electron'
    if (isMobileShell.value) {
      const declared = win?.ClubOA?.platform?.()
      if (declared === 'android' || declared === 'harmony') return declared
      return win?.ClubOA?.vendor?.() === 'huawei' ? 'harmony' : 'android'
    }
    if (isIOSPWA.value) return 'ios-pwa'
    return 'browser'
  })

  /** 是否运行在原生/桌面壳中。 */
  const isShell = computed(
    () => platform.value === 'android' || platform.value === 'harmony' || platform.value === 'electron',
  )

  /** 推送厂商（移动壳；其余返回 null）。 */
  const vendor = computed(() => {
    if (platform.value === 'android' || platform.value === 'harmony') {
      return win?.ClubOA?.vendor?.() ?? null
    }
    if (platform.value === 'electron') return 'desktop'
    return null
  })

  /** 清除服务器配置并回到设置页（壳内可用）。 */
  async function switchServer(): Promise<void> {
    if (!win) return
    if (win.clubOA) {
      await win.clubOA.clearServer()
      return
    }
    if (win.ClubOA?.switchServer) {
      win.ClubOA.switchServer()
    }
  }

  /** 同步未读角标（桌面壳托盘/Dock）。 */
  async function setUnreadCount(count: number): Promise<void> {
    if (win?.clubOA?.setUnreadCount) {
      await win.clubOA.setUnreadCount(count)
    }
  }

  /** 获取厂商推送 token（壳内可用）。 */
  async function getPushToken(): Promise<string | null> {
    const bridge = win?.ClubOA
    if (!bridge?.pushToken) return null
    return (await bridge.pushToken()) ?? null
  }

  return {
    platform,
    isShell,
    isElectron,
    isMobileShell,
    isIOSPWA,
    vendor,
    switchServer,
    setUnreadCount,
    getPushToken,
  }
}
