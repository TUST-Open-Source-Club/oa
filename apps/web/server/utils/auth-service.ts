/**
 * BFF 与 auth 服务的调用封装。
 *
 * 浏览器永远拿不到 Refresh Token：由 BFF 写入 httpOnly Cookie；
 * Access Token 只返回给前端内存保管。
 */
import type { H3Event } from 'h3'

/** 刷新令牌 Cookie 名。 */
export const REFRESH_COOKIE = 'club_refresh'
/** 刷新令牌 Cookie 有效期（秒，30 天，与 auth 服务一致）。 */
export const REFRESH_MAX_AGE = 60 * 60 * 24 * 30

/** 调用 auth 服务（POST），把后端 ProblemDetails 转成 H3 错误。 */
export async function callAuthService<T>(
  event: H3Event,
  path: string,
  body?: unknown,
): Promise<T> {
  const base = useRuntimeConfig(event).authServiceUrl
  try {
    return await $fetch<T>(`${base}${path}`, { method: 'POST', body })
  } catch (error: unknown) {
    const fetchError = error as {
      response?: { status?: number }
      data?: { code?: string } | null
    }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: fetchError.data?.code ?? 'AUTH_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
}

/** 写入刷新令牌 Cookie（httpOnly + SameSite=Lax）。 */
export function setRefreshCookie(event: H3Event, refreshToken: string) {
  setCookie(event, REFRESH_COOKIE, refreshToken, {
    httpOnly: true,
    sameSite: 'lax',
    secure: !import.meta.dev,
    path: '/',
    maxAge: REFRESH_MAX_AGE,
  })
}

/** 清除刷新令牌 Cookie。 */
export function clearRefreshCookie(event: H3Event) {
  deleteCookie(event, REFRESH_COOKIE, { path: '/' })
}
