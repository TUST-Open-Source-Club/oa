/** 任务附件签名下载：token 原样转给 task 服务（卡片封面用）。 */
export default defineEventHandler(async (event) => {
  const id = getRouterParam(event, 'id') ?? ''
  const token = String(getQuery(event).token ?? '')
  if (!id || !token) {
    throw createError({ statusCode: 400, statusMessage: 'TASK_FILE_BAD_REQUEST' })
  }
  const base = useRuntimeConfig(event).taskServiceUrl
  const upstream = await fetch(
    `${base}/api/v1/task/attachments/${encodeURIComponent(id)}/file?token=${encodeURIComponent(token)}`,
  )
  if (!upstream.ok) {
    throw createError({
      statusCode: upstream.status,
      statusMessage: upstream.status === 401 ? 'TASK_FILE_TOKEN_INVALID' : 'TASK_FILE_ERROR',
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
