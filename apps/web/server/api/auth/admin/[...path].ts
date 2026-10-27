/** 管理员接口代理（透传 Authorization 与查询串，JSON 请求/响应）。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const path = getRouterParam(event, 'path') ?? ''
  const method = event.method
  const query = getQuery(event)
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null) params.set(key, String(value))
  }
  const suffix = params.toString() ? `?${params.toString()}` : ''
  const body = ['POST', 'PUT', 'PATCH'].includes(method)
    ? await readBody(event).catch(() => undefined)
    : undefined
  try {
    return await $fetch(`${useRuntimeConfig(event).authServiceUrl}/api/v1/auth/admin/${path}${suffix}`, {
      method,
      body,
      headers: { authorization },
    })
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: { code?: string } | null }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: fetchError.data?.code ?? 'AUTH_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
