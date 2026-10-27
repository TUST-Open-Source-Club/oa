/** 用户查询代理：`?q=` 关键字搜索，`?ids=` 批量按 ID（供选人组件与 IM 展示）。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const query = getQuery(event)
  const params = new URLSearchParams()
  let path = '/api/v1/auth/users/search'
  if (query.ids !== undefined) {
    path = '/api/v1/auth/users'
    params.set('ids', String(query.ids))
  } else {
    params.set('q', String(query.q ?? ''))
  }
  try {
    return await $fetch(`${useRuntimeConfig(event).authServiceUrl}${path}?${params.toString()}`, {
      headers: { authorization },
    })
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'AUTH_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
