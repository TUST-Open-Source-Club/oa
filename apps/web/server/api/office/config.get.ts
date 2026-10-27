/** OnlyOffice 预览配置代理：把 drive 的 office-config 透传给浏览器。 */
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
    return await $fetch(
      `${base}/api/v1/drive/spaces/${spaceId}/nodes/${nodeId}/office-config`,
      { headers: { authorization } },
    )
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: 'OFFICE_SERVICE_ERROR',
      data: fetchError.data ?? null,
    })
  }
})
