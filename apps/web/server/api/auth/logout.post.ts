/** BFF 登出：吊销刷新令牌并清除 Cookie（即使失败也清理本地 Cookie）。 */
export default defineEventHandler(async (event) => {
  const refreshToken = getCookie(event, REFRESH_COOKIE)
  if (refreshToken) {
    try {
      await callAuthService(event, '/api/v1/auth/logout', { refreshToken })
    } catch {
      // 登出以清理本地会话为准，忽略服务端吊销失败（令牌会自然过期）
    }
  }
  clearRefreshCookie(event)
  return { ok: true }
})
