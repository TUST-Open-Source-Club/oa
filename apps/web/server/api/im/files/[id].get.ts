/**
 * IM 媒体下载代理：`<img>`/`<a download>` 无法携带 Authorization，
 * 这里把签名 token 原样转给 im 服务并回传字节。
 */
export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, 'id') ?? ''
  const token = String(getQuery(event).token ?? '')
  if (!id || !token) {
    throw createError({ statusCode: 400, statusMessage: 'IM_FILE_BAD_REQUEST' })
  }
  const base = useRuntimeConfig(event).imServiceUrl
  try {
    const upstream = await $fetch.raw(
      `${base}/api/v1/im/files/${encodeURIComponent(id)}?token=${encodeURIComponent(token)}`,
      { responseType: 'arrayBuffer' },
    )
    setResponseHeader(
      event,
      'content-type',
      upstream.headers.get('content-type') ?? 'application/octet-stream',
    )
    setResponseHeader(event, 'cache-control', 'private, max-age=3600')
    return new Uint8Array(upstream._data as ArrayBuffer)
  } catch (error: unknown) {
    const status = (error as { response?: { status?: number } }).response?.status ?? 502
    throw createError({
      statusCode: status,
      statusMessage: status === 401 ? 'IM_FILE_TOKEN_INVALID' : 'IM_FILE_ERROR',
    })
  }
})
