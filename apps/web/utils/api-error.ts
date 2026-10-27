import type { ProblemDetails } from '@club-oa/core'

/**
 * 从 ofetch/FetchError 中提取后端问题详情。
 * 兼容两种形态：BFF 直传的 ProblemDetails，以及 H3 `createError` 包装后的 `data.data`。
 */
export function problemOf(error: unknown): ProblemDetails | null {
  const data = (error as { data?: unknown } | null)?.data
  if (!data || typeof data !== 'object') return null
  const nested = (data as { data?: unknown }).data
  if (nested && typeof nested === 'object') return nested as ProblemDetails
  if ('detail' in data || 'code' in data) return data as ProblemDetails
  return null
}

/** 面向用户的可读错误消息（优先业务 detail，其次 title，最后兜底文案）。 */
export function apiErrorMessage(error: unknown, fallback = '请求失败，请稍后重试'): string {
  const problem = problemOf(error)
  if (problem?.detail) return problem.detail
  if (problem?.title) return problem.title
  return fallback
}
