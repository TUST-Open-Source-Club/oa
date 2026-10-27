/** 活动公开接口代理（无需登录）：活动详情、发送验证码、提交报名、状态查询。 */
export default defineEventHandler(async (event) => {
  const path = getRouterParam(event, 'path') ?? ''
  const method = event.method
  const query = getQuery(event)
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null) params.set(key, String(value))
  }
  const body = ['POST', 'PUT', 'PATCH'].includes(method)
    ? await readBody(event).catch(() => undefined)
    : undefined
  const suffix = params.toString() ? `?${params.toString()}` : ''
  try {
    return await $fetch(`${useRuntimeConfig(event).eventServiceUrl}/api/v1/event/public/${path}${suffix}`, {
      method,
      body,
    })
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'EVENT_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
