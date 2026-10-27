/** 公开分享下载代理（无需登录；密码通过查询参数传递）。 */
export default defineEventHandler(async (event) => {
  const token = getRouterParam(event, 'token')
  if (!token) throw createError({ statusCode: 400, statusMessage: 'MISSING_TOKEN' })
  const { password } = getQuery(event)
  const query = password ? `?password=${encodeURIComponent(String(password))}` : ''
  const base = useRuntimeConfig(event).driveServiceUrl
  try {
    const response = await $fetch.raw(
      `${base}/api/v1/drive/public/shares/${token}/download${query}`,
      { responseType: 'arrayBuffer' },
    )
    const contentType = response.headers.get('content-type') ?? 'application/octet-stream'
    const disposition = response.headers.get('content-disposition')
    setResponseHeader(event, 'content-type', contentType)
    if (disposition) setResponseHeader(event, 'content-disposition', disposition)
    // 必须显式转 Buffer：ArrayBuffer 直接返回会被 Nitro 序列化成 {}
    return Buffer.from(response._data as ArrayBuffer)
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'SHARE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
