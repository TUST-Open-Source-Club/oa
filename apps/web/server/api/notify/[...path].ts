/**
 * notify 服务 BFF 代理：透传 Authorization，强制登录后访问。
 * 路径示例：/api/notify/notifications、/api/notify/notifications/{id}/read
 */
export default defineEventHandler(async (event) => {
  const path = getRouterParam(event, 'path') ?? ''
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const method = event.method
  const query = getQuery(event)
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null) params.set(key, String(value))
  }
  const body = ['POST', 'PUT', 'PATCH'].includes(method)
    ? await readBody(event).catch(() => undefined)
    : undefined

  const base = useRuntimeConfig(event).notifyServiceUrl
  const suffix = params.toString() ? `?${params.toString()}` : ''
  try {
    return await $fetch(`${base}/api/v1/notify/${path}${suffix}`, {
      method,
      body,
      headers: { authorization },
    })
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'NOTIFY_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
