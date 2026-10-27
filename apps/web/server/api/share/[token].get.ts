/** 公开分享信息代理（无需登录，支持密码查询参数）。 */
export default defineEventHandler(async (event) => {
  const token = getRouterParam(event, 'token')
  if (!token) throw createError({ statusCode: 400, statusMessage: 'MISSING_TOKEN' })
  const { password } = getQuery(event)
  const query = password ? `?password=${encodeURIComponent(String(password))}` : ''
  const base = useRuntimeConfig(event).driveServiceUrl
  try {
    return await $fetch(`${base}/api/v1/drive/public/shares/${token}${query}`)
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'SHARE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
