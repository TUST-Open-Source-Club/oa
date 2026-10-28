/** 文档图片下载代理：签名 token 原样转给 doc 服务，便于 <img> 直接引用。 */
export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, 'id') ?? ''
  const token = String(getQuery(event).token ?? '')
  if (!id || !token) {
    throw createError({ statusCode: 400, statusMessage: 'DOC_FILE_BAD_REQUEST' })
  }
  const base = useRuntimeConfig(event).docServiceUrl
  try {
    const upstream = await $fetch.raw(
      `${base}/api/v1/doc/files/${encodeURIComponent(id)}?token=${encodeURIComponent(token)}`,
      { responseType: 'arrayBuffer' },
    )
    setResponseHeader(
      event,
      'content-type',
      upstream.headers.get('content-type') ?? 'application/octet-stream',
    )
    setResponseHeader(event, 'cache-control', 'private, max-age=86400')
    return new Uint8Array(upstream._data as ArrayBuffer)
  } catch (error: unknown) {
    const status = (error as { response?: { status?: number } }).response?.status ?? 502
    throw createError({
      statusCode: status,
      statusMessage: status === 401 ? 'DOC_FILE_TOKEN_INVALID' : 'DOC_FILE_ERROR',
    })
  }
})
