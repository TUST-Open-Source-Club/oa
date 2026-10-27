import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { useAuthStore } from './auth'

/** 构造返回固定 JSON 的 fetch 替身。 */
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

const sampleUser = {
  id: 'u1',
  username: 'alice',
  email: 'alice@club.example.com',
  nickname: 'Alice',
  roles: ['member'],
  status: 'active',
}

describe('useAuthStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.unstubAllGlobals()
  })

  it('登录成功后保存令牌与用户', async () => {
    stubFetchSequence([
      { body: { accessToken: 'access-1', expiresIn: 900, user: sampleUser } },
    ])
    const auth = useAuthStore()
    await auth.login('alice', 'Alice1234')
    expect(auth.accessToken).toBe('access-1')
    expect(auth.user?.username).toBe('alice')
    expect(auth.isAuthenticated).toBe(true)
    expect(auth.isAdmin).toBe(false)
  })

  it('登录失败抛出错误且保持未登录', async () => {
    stubFetchSequence([
      {
        status: 401,
        body: {
          type: 't',
          title: 'Unauthorized',
          status: 401,
          detail: '账号或密码错误',
          code: 'AUTH_INVALID_CREDENTIALS',
        },
      },
    ])
    const auth = useAuthStore()
    await expect(auth.login('alice', 'wrong')).rejects.toThrow()
    expect(auth.isAuthenticated).toBe(false)
  })

  it('restore 成功后依次调用 refresh 与 me', async () => {
    stubFetchSequence([
      { body: { accessToken: 'access-2', expiresIn: 900 } },
      { body: { ...sampleUser, roles: ['admin'] } },
    ])
    const auth = useAuthStore()
    await auth.restore()
    expect(auth.accessToken).toBe('access-2')
    expect(auth.user?.roles).toContain('admin')
    expect(auth.isAdmin).toBe(true)
    expect(auth.ready).toBe(true)
  })

  it('restore 失败时清空状态并标记 ready', async () => {
    stubFetchSequence([{ status: 401, body: { code: 'NO_REFRESH_TOKEN' } }])
    const auth = useAuthStore()
    await auth.restore()
    expect(auth.isAuthenticated).toBe(false)
    expect(auth.ready).toBe(true)
  })

  it('logout 清空本地状态且忽略服务端错误', async () => {
    stubFetchSequence([{ status: 502, body: { code: 'AUTH_SERVICE_ERROR' } }])
    const auth = useAuthStore()
    auth.accessToken = 'access-1'
    auth.user = sampleUser
    await auth.logout()
    expect(auth.accessToken).toBe('')
    expect(auth.user).toBeNull()
  })

  it('authHeaders 按登录态返回鉴权头', () => {
    const auth = useAuthStore()
    expect(auth.authHeaders()).toEqual({})
    auth.accessToken = 'access-1'
    expect(auth.authHeaders()).toEqual({ authorization: 'Bearer access-1' })
  })
})
