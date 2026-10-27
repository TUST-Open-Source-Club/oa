/** 任务附件上传代理：透传原始字节（附件最大 50MB）。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const { taskId, name, mime } = getQuery(event) as { taskId?: string; name?: string; mime?: string }
  if (!taskId || !name) {
    throw createError({ statusCode: 400, statusMessage: 'MISSING_PARAMS' })
  }
  const body = await readRawBody(event, false)
  const contentType = mime || 'application/octet-stream'
  try {
    return await $fetch(
      `${useRuntimeConfig(event).taskServiceUrl}/api/v1/task/tasks/${taskId}/attachments?name=${encodeURIComponent(name)}&mime=${encodeURIComponent(contentType)}`,
      { method: 'POST', body, headers: { authorization, 'content-type': contentType } },
    )
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'TASK_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
