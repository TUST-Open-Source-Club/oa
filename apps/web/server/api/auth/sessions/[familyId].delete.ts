/** BFF 登出指定设备（吊销该设备的刷新令牌链）。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const familyId = getRouterParam(event, 'familyId') ?? ''
  const base = useRuntimeConfig(event).authServiceUrl
  try {
    await $fetch(`${base}/api/v1/auth/sessions/${encodeURIComponent(familyId)}`, {
      method: 'DELETE',
      headers: { authorization },
    })
    setResponseStatus(event, 204)
    return null
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      data: fetchError.data ?? null,
    })
  }
})
