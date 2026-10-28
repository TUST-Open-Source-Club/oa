/** BFF 登录设备列表：透传 Authorization，并注入 refresh Cookie 以标记当前设备。 */
export default defineEventHandler(async (event) => {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const refresh = getCookie(event, REFRESH_COOKIE)
  const base = useRuntimeConfig(event).authServiceUrl
  try {
    return await $fetch(`${base}/api/v1/auth/sessions`, {
      headers: {
        authorization,
        ...(refresh ? { 'x-refresh-token': refresh } : {}),
      },
    })
  } catch (error: unknown) {
    const fetchError = error as { response?: { status?: number }; data?: unknown }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      data: fetchError.data ?? null,
    })
  }
})
