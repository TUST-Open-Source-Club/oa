/** 文档图片下载代理：签名 token 原样转给 doc 服务，便于 <img> 直接引用。 */
export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, 'id') ?? ''
  const token = String(getQuery(event).token ?? '')
  if (!id || !token) {
    throw createError({ statusCode: 400, statusMessage: 'DOC_FILE_BAD_REQUEST' })
  }
  const base = useRuntimeConfig(event).docServiceUrl
  const upstream = await fetch(
    `${base}/api/v1/doc/files/${encodeURIComponent(id)}?token=${encodeURIComponent(token)}`,
  )
  if (!upstream.ok) {
    throw createError({
      statusCode: upstream.status,
      statusMessage: upstream.status === 401 ? 'DOC_FILE_TOKEN_INVALID' : 'DOC_FILE_ERROR',
    })
  }
  setResponseHeader(
    event,
    'content-type',
    upstream.headers.get('content-type') ?? 'application/octet-stream',
  )
  setResponseHeader(event, 'cache-control', 'private, max-age=86400')
  return Buffer.from(await upstream.arrayBuffer())
})
