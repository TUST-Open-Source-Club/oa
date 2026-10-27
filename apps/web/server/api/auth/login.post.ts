/** BFF 登录：代理 auth 服务，刷新令牌写 httpOnly Cookie，只回传 Access Token 与用户信息。 */
export default defineEventHandler(async (event) => {
  const body = await readBody<{ identifier: string; password: string; deviceId?: string }>(event)
  const result = await callAuthService<{
    accessToken: string
    refreshToken: string
    expiresIn: number
    user: unknown
  }>(event, '/api/v1/auth/login', body)

  setRefreshCookie(event, result.refreshToken)
  return {
    accessToken: result.accessToken,
    expiresIn: result.expiresIn,
    user: result.user,
  }
})
