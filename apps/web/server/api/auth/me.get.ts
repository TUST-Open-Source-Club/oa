/** BFF 用户信息：透传 Authorization 到 auth 服务。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const base = useRuntimeConfig(event).authServiceUrl
  try {
    return await $fetch(`${base}/api/v1/auth/me`, { headers: { authorization } })
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      data: fetchError.data ?? null,
    })
  }
})
