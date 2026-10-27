import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { useAuthStore } from './auth'
import { useNotificationsStore } from './notifications'

/** 按顺序返回响应的 fetch 替身。 */
function stubFetchSequence(responses: Array<{ status?: number; body: unknown }>) {
  const fn = vi.fn(async () => {
    const next = responses.shift()
    if (!next) throw new Error('unexpected fetch call')
    return new Response(JSON.stringify(next.body), {
      status: next.status ?? 200,
      headers: { 'content-type': 'application/json' },
    })
  })
  vi.stubGlobal('fetch', fn)
  return fn
}

function item(id: string, read = false) {
  return {
    id,
    eventType: 'task.assigned',
    title: '标题',
    body: '内容',
    priority: 'high',
    url: '/tasks/1',
    read,
    createdAt: '2026-09-13T10:00:00Z',
  }
}

describe('useNotificationsStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.unstubAllGlobals()
    const auth = useAuthStore()
    auth.accessToken = 'access-1'
    auth.user = {
      id: 'u1',
      username: 'alice',
      email: 'a@b.cn',
      nickname: 'Alice',
      roles: ['member'],
      status: 'active',
    }
  })

  it('fetchUnreadCount 更新未读数与 hasUnread', async () => {
    stubFetchSequence([{ body: { count: 3 } }])
    const store = useNotificationsStore()
    expect(store.hasUnread).toBe(false)
    await store.fetchUnreadCount()
    expect(store.unreadCount).toBe(3)
    expect(store.hasUnread).toBe(true)
  })

  it('fetchList 支持首次加载与追加', async () => {
    stubFetchSequence([
      { body: { items: [item('n1')], nextCursor: 'cursor-1' } },
      { body: { items: [item('n2')], nextCursor: null } },
    ])
    const store = useNotificationsStore()
    await store.fetchList()
    expect(store.items.map((entry) => entry.id)).toEqual(['n1'])
    expect(store.nextCursor).toBe('cursor-1')

    await store.fetchList({ append: true })
    expect(store.items.map((entry) => entry.id)).toEqual(['n1', 'n2'])
    expect(store.nextCursor).toBeNull()
  })

  it('markRead 同步本地状态并递减未读数', async () => {
    stubFetchSequence([{ body: { updated: 1 } }])
    const store = useNotificationsStore()
    store.items = [item('n1'), item('n2')]
    store.unreadCount = 2
    await store.markRead('n1')
    expect(store.items[0].read).toBe(true)
    expect(store.items[1].read).toBe(false)
    expect(store.unreadCount).toBe(1)
  })

  it('markAllRead 全部置为已读并清零', async () => {
    stubFetchSequence([{ body: { updated: 2 } }])
    const store = useNotificationsStore()
    store.items = [item('n1'), item('n2')]
    store.unreadCount = 2
    await store.markAllRead()
    expect(store.items.every((entry) => entry.read)).toBe(true)
    expect(store.unreadCount).toBe(0)
  })
})
