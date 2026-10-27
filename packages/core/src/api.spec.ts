import { describe, expect, it, vi } from 'vitest'

import { ApiError, createApiClient } from './api'

/** 构造返回固定 JSON 的 fetch 替身。 */
function jsonFetch(status: number, body: unknown, headers: Record<string, string> = {}) {
  return vi.fn(async () =>
    new Response(body === undefined ? null : JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json', ...headers },
    }),
  ) as unknown as typeof fetch
}

describe('createApiClient', () => {
  it('GET 请求拼接查询参数并解析 JSON', async () => {
    const fetchImpl = jsonFetch(200, { items: [1, 2], nextCursor: null })
    const api = createApiClient({ baseUrl: 'https://api.test', fetchImpl })
    const result = await api.get<{ items: number[] }>('/users', { query: { q: 'a b', page: 2, skip: undefined } })
    expect(result.items).toEqual([1, 2])
    const calledUrl = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][0] as string
    expect(calledUrl).toBe('https://api.test/users?q=a+b&page=2')
  })

  it('POST 发送 JSON 并携带 Bearer 令牌', async () => {
    const fetchImpl = jsonFetch(200, { ok: true })
    const api = createApiClient({
      baseUrl: 'https://api.test',
      getToken: () => 'token-1',
      fetchImpl,
    })
    await api.post('/login', { identifier: 'a' })
    const init = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls[0][1] as RequestInit
    expect(init.body).toBe(JSON.stringify({ identifier: 'a' }))
    expect((init.headers as Record<string, string>).authorization).toBe('Bearer token-1')
  })

  it('204 返回 undefined', async () => {
    const fetchImpl = vi.fn(async () => new Response(null, { status: 204 })) as unknown as typeof fetch
    const api = createApiClient({ baseUrl: 'https://api.test', fetchImpl })
    await expect(api.delete('/logout')).resolves.toBeUndefined()
  })

  it('非 2xx 抛出 ApiError 并保留业务错误码与字段错误', async () => {
    const fetchImpl = jsonFetch(422, {
      type: 'https://club-oa.local/errors/AUTH_VALIDATION',
      title: 'Unprocessable Entity',
      status: 422,
      detail: '密码不符合要求',
      code: 'AUTH_VALIDATION',
      errors: [{ field: 'password', message: 'password.too_short' }],
    })
    const api = createApiClient({ baseUrl: 'https://api.test', fetchImpl })
    const error = await api.post('/activate', {}).catch((err: unknown) => err)
    expect(error).toBeInstanceOf(ApiError)
    const apiError = error as ApiError
    expect(apiError.status).toBe(422)
    expect(apiError.code).toBe('AUTH_VALIDATION')
    expect(apiError.details[0]).toEqual({ field: 'password', message: 'password.too_short' })
  })

  it('非 JSON 错误响应降级为 HTTP_ERROR', async () => {
    const fetchImpl = vi.fn(
      async () => new Response('Bad Gateway', { status: 502, statusText: 'Bad Gateway' }),
    ) as unknown as typeof fetch
    const api = createApiClient({ baseUrl: 'https://api.test', fetchImpl })
    const error = (await api.get('/x').catch((err: unknown) => err)) as ApiError
    expect(error).toBeInstanceOf(ApiError)
    expect(error.code).toBe('HTTP_ERROR')
    expect(error.status).toBe(502)
  })

  it('put/patch/request 方法与自定义请求头可用', async () => {
    const fetchImpl = jsonFetch(200, { ok: true })
    const api = createApiClient({ baseUrl: 'https://api.test', fetchImpl })
    await api.put('/me', { nickname: 'A' })
    await api.patch('/me', { bio: 'B' })
    await api.request('GET', '/raw', undefined, { headers: { 'x-trace-id': 'trace-1' } })
    const calls = (fetchImpl as unknown as ReturnType<typeof vi.fn>).mock.calls
    expect(calls[0][0]).toBe('https://api.test/me')
    expect((calls[2][1] as RequestInit).headers).toMatchObject({ 'x-trace-id': 'trace-1' })
  })

  it('网络异常向上传递', async () => {
    const fetchImpl = vi.fn(async () => {
      throw new TypeError('fetch failed')
    }) as unknown as typeof fetch
    const api = createApiClient({ baseUrl: 'https://api.test', fetchImpl })
    await expect(api.get('/x')).rejects.toThrow('fetch failed')
  })
})
