import { afterEach, describe, expect, it, vi } from 'vitest'

import { useShell } from './useShell'

/** 清理全局桩。 */
afterEach(() => {
  vi.unstubAllGlobals()
})

/** 安装 window/navigator 桩。 */
function stubEnv(extra: { clubOA?: unknown; ClubOA?: unknown; ua?: string; standalone?: boolean }) {
  vi.stubGlobal('window', {
    clubOA: extra.clubOA,
    ClubOA: extra.ClubOA,
  })
  vi.stubGlobal('navigator', {
    userAgent: extra.ua ?? 'Mozilla/5.0',
    standalone: extra.standalone,
  })
}

describe('useShell', () => {
  it('无 window 时视为浏览器', async () => {
    const shell = useShell()
    expect(shell.platform.value).toBe('browser')
    expect(shell.isShell.value).toBe(false)
    expect(shell.vendor.value).toBeNull()
    await expect(shell.setUnreadCount(1)).resolves.toBeUndefined()
    await expect(shell.switchServer()).resolves.toBeUndefined()
  })

  it('检测 Electron 壳', async () => {
    const clearServer = vi.fn(async () => undefined)
    const setUnreadCount = vi.fn(async () => undefined)
    stubEnv({
      clubOA: { platform: 'electron', vendor: 'desktop', clearServer, notify: vi.fn(), setUnreadCount },
    })
    const shell = useShell()
    expect(shell.platform.value).toBe('electron')
    expect(shell.isShell.value).toBe(true)
    expect(shell.vendor.value).toBe('desktop')
    await shell.switchServer()
    expect(clearServer).toHaveBeenCalledOnce()
    await shell.setUnreadCount(3)
    expect(setUnreadCount).toHaveBeenCalledWith(3)
  })

  it('检测 Android 壳并读取推送能力', async () => {
    const pushToken = vi.fn(() => 'tok-1')
    stubEnv({ ClubOA: { platform: () => 'android', vendor: () => 'xiaomi', pushToken } })
    const shell = useShell()
    expect(shell.platform.value).toBe('android')
    expect(shell.vendor.value).toBe('xiaomi')
    await expect(shell.getPushToken()).resolves.toBe('tok-1')
  })

  it('检测 HarmonyOS 壳并调用 switchServer', () => {
    const switchServer = vi.fn()
    stubEnv({ ClubOA: { platform: () => 'harmony', vendor: () => 'huawei', switchServer } })
    const shell = useShell()
    expect(shell.platform.value).toBe('harmony')
    shell.switchServer()
    expect(switchServer).toHaveBeenCalledOnce()
  })

  it('未知平台时按 vendor 猜测', () => {
    stubEnv({ ClubOA: { vendor: () => 'huawei' } })
    expect(useShell().platform.value).toBe('harmony')
    vi.unstubAllGlobals()
    stubEnv({ ClubOA: { vendor: () => 'oppo' } })
    expect(useShell().platform.value).toBe('android')
  })

  it('检测 iOS 主屏 PWA', () => {
    stubEnv({ ua: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)', standalone: true })
    const shell = useShell()
    expect(shell.platform.value).toBe('ios-pwa')
    expect(shell.isShell.value).toBe(false)
  })
})
