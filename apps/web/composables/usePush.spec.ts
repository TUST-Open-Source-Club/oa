import { afterEach, describe, expect, it, vi } from 'vitest'

import { topicFor, usePush } from './usePush'

/** 清理全局桩。 */
afterEach(() => {
  vi.unstubAllGlobals()
})

describe('topicFor', () => {
  it('与 notify 服务 topic_for 对齐（u_ + 无连字符 uuid）', () => {
    expect(topicFor('0198f0b0-1234-7000-8000-abcdefabcdef')).toBe('u_0198f0b0123470008000abcdefabcdef')
  })
})

describe('usePush', () => {
  it('无 Notification 环境不支持', () => {
    const push = usePush('http://ntfy.local')
    expect(push.supported).toBe(false)
    expect(push.enabled.value).toBe(false)
    expect(push.topic.value).toBeNull()
    expect(push.subscribeUrl.value).toBeNull()
  })

  it('授权后可计算订阅地址', async () => {
    const requestPermission = vi.fn(async () => 'granted' as NotificationPermission)
    vi.stubGlobal('window', { Notification: { permission: 'default', requestPermission } })
    const push = usePush('http://ntfy.local')
    push.setUser('0198f0b0-1234-7000-8000-abcdefabcdef')
    await push.enable()
    expect(push.permission.value).toBe('granted')
    expect(push.enabled.value).toBe(true)
    expect(push.topic.value).toBe('u_0198f0b0123470008000abcdefabcdef')
    expect(push.subscribeUrl.value).toBe('http://ntfy.local/u_0198f0b0123470008000abcdefabcdef')
  })

  it('拒绝授权时报错并保持禁用', async () => {
    vi.stubGlobal('window', {
      Notification: { permission: 'default', requestPermission: vi.fn(async () => 'denied') },
    })
    const push = usePush()
    await expect(push.enable()).rejects.toThrow('未获得通知权限')
    expect(push.enabled.value).toBe(false)
    push.disable()
    expect(push.permission.value).toBe('default')
  })
})
