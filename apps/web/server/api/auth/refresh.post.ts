/** BFF 刷新：凭 httpOnly Cookie 中的刷新令牌换发新的 Access Token。 */
export default defineEventHandler(async (event) => {
  const refreshToken = getCookie(event, REFRESH_COOKIE)
  if (!refreshToken) {
    throw createError({ statusCode: 401, statusMessage: 'NO_REFRESH_TOKEN' })
  }
  const result = await callAuthService<{
    accessToken: string
    refreshToken: string
    expiresIn: number
  }>(event, '/api/v1/auth/refresh', { refreshToken })

  setRefreshCookie(event, result.refreshToken)
  return { accessToken: result.accessToken, expiresIn: result.expiresIn }
})
