/** 任务附件下载代理：透传 Authorization，返回原始字节。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const { taskId, attachmentId } = getQuery(event) as { taskId?: string; attachmentId?: string }
  if (!taskId || !attachmentId) {
    throw createError({ statusCode: 400, statusMessage: 'MISSING_PARAMS' })
  }
  try {
    const response = await $fetch.raw(
      `${useRuntimeConfig(event).taskServiceUrl}/api/v1/task/tasks/${taskId}/attachments/${attachmentId}/download`,
      { headers: { authorization }, responseType: 'arrayBuffer' },
    )
    setResponseHeader(event, 'content-type', response.headers.get('content-type') ?? 'application/octet-stream')
    const disposition = response.headers.get('content-disposition')
    if (disposition) setResponseHeader(event, 'content-disposition', disposition)
    return Buffer.from(response._data as ArrayBuffer)
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number } }
    throw createError({ statusCode: fetchError.response?.status ?? 502 })
  }
})
