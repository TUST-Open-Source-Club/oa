/** 文档公开分享读取：免登录，密码通过查询参数透传。 */
export default defineEventHandler(async (event) => {
  const token = getRouterParam(event, 'token') ?? ''
  const password = getQuery(event).password
  const base = useRuntimeConfig(event).docServiceUrl
  try {
    return await $fetch(`${base}/api/v1/doc/public/${encodeURIComponent(token)}`, {
      query: password ? { password: String(password) } : {},
    })
  } catch (error: unknown) {
    const fetchError = error as {
      response?: { status?: number }
      data?: { code?: string } | null
    }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: fetchError.data?.code ?? 'DOC_SHARE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
