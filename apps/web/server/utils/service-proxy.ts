/**
 * 通用服务代理：透传 Authorization 与查询串，JSON 请求/响应。
 * 上传大文件等二进制场景使用专用路由（见 drive/download.get.ts）。
 */
import type { H3Event } from 'h3'

/**
 * 代理 JSON 请求到内部服务。
 *
 * @param event h3 事件
 * @param baseUrl 内部服务地址（如 http://drive:8087）
 * @param apiPrefix 服务 API 前缀（如 /api/v1/drive）
 * @param errorTag 错误标记（ProblemDetails 透传失败时使用）
 */
export async function proxyServiceJson(
  event: H3Event,
  baseUrl: string,
  apiPrefix: string,
  errorTag: string,
): Promise<unknown> {
  const authorization = getRequestHeader(event, 'authorization')
  if (!authorization) {
    throw createError({ statusCode: 401, statusMessage: 'MISSING_TOKEN' })
  }
  const path = getRouterParam(event, 'path') ?? ''
  const method = event.method
  const query = getQuery(event)
  const params = new URLSearchParams()
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined && value !== null) params.set(key, String(value))
  }
  const body = ['POST', 'PUT', 'PATCH'].includes(method)
    ? await readBody(event).catch(() => undefined)
    : undefined
  const suffix = params.toString() ? `?${params.toString()}` : ''
  try {
    return await $fetch(`${baseUrl}${apiPrefix}/${path}${suffix}`, {
      method,
      body,
      headers: { authorization },
    })
  } catch (error: unknown) {
    const fetchError = error as {
      response?: { status?: number }
      data?: { code?: string } | null
    }
    throw createError({
      statusCode: fetchError.response?.status ?? 502,
      statusMessage: fetchError.data?.code ?? errorTag,
      data: fetchError.data ?? null,
    })
  }
}
