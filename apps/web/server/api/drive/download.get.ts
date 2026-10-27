/**
 * 文件下载代理：透传 Authorization，返回原始字节。
 * 前端以 fetch + Blob 方式调用（携带内存中的 Access Token）。
 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const { spaceId, nodeId } = getQuery(event) as { spaceId?: string; nodeId?: string }
  if (!spaceId || !nodeId) {
    throw createError({ statusCode: 400, statusMessage: 'MISSING_PARAMS' })
  }
  const base = useRuntimeConfig(event).driveServiceUrl
  try {
    const response = await $fetch.raw(
      `${base}/api/v1/drive/spaces/${spaceId}/nodes/${nodeId}/download`,
      { headers: { authorization }, responseType: 'arrayBuffer' },
    )
    const contentType = response.headers.get('content-type') ?? 'application/octet-stream'
    const disposition = response.headers.get('content-disposition')
    setResponseHeader(event, 'content-type', contentType)
    if (disposition) setResponseHeader(event, 'content-disposition', disposition)
    // 必须显式转 Buffer：ArrayBuffer 直接返回会被 Nitro 序列化成 {}
    return Buffer.from(response._data as ArrayBuffer)
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number } }
    throw createError({ statusCode: fetchError.response?.status ?? 502 })
  }
})
