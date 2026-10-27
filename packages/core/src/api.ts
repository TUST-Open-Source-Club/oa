/**
 * 统一 API 客户端：与后端 RFC 7807 错误模型对齐，供 Web BFF 与 Tauri 客户端复用。
 */

/** RFC 7807 Problem Details 响应体。 */
export interface ProblemDetails {
  /** 错误类型 URI */
  type: string
  /** 状态短语 */
  title: string
  /** HTTP 状态码 */
  status: number
  /** 错误描述 */
  detail: string
  /** 业务错误码 */
  code: string
  /** 字段级错误（422） */
  errors?: Array<{ field: string; message: string }>
}

/** 后端错误包装：保留状态码与业务错误码，便于 UI 精准提示。 */
export class ApiError extends Error {
  readonly status: number
  readonly code: string
  readonly details: Array<{ field: string; message: string }>

  constructor(problem: ProblemDetails) {
    super(problem.detail || problem.title)
    this.name = 'ApiError'
    this.status = problem.status
    this.code = problem.code
    this.details = problem.errors ?? []
  }
}

/** 客户端创建参数。 */
export interface ApiClientOptions {
  /** 服务基地址（不含尾斜杠），如 https://oa.example.com/api/v1/auth */
  baseUrl: string
  /** 获取 Access Token（无登录态返回 null） */
  getToken?: () => string | null
  /** 自定义 fetch（测试用） */
  fetchImpl?: typeof fetch
}

/** 请求选项。 */
export interface RequestOptions {
  /** 查询参数（undefined 的键会被忽略） */
  query?: Record<string, string | number | boolean | undefined>
  /** 额外请求头 */
  headers?: Record<string, string>
}

/**
 * 创建 API 客户端。
 *
 * 约定：JSON 请求/响应；204 返回 undefined；非 2xx 抛出 {@link ApiError}。
 */
export function createApiClient(options: ApiClientOptions) {
  const fetchImpl = options.fetchImpl ?? fetch

  /** 拼接查询串。 */
  function buildUrl(path: string, query?: RequestOptions['query']): string {
    if (!query) return `${options.baseUrl}${path}`
    const params = new URLSearchParams()
    for (const [key, value] of Object.entries(query)) {
      if (value !== undefined) params.set(key, String(value))
    }
    const suffix = params.toString()
    return suffix ? `${options.baseUrl}${path}?${suffix}` : `${options.baseUrl}${path}`
  }

  /** 执行请求并解析响应。 */
  async function request<T>(
    method: string,
    path: string,
    body?: unknown,
    requestOptions: RequestOptions = {},
  ): Promise<T> {
    const headers: Record<string, string> = { 'content-type': 'application/json', ...requestOptions.headers }
    const token = options.getToken?.()
    if (token) headers.authorization = `Bearer ${token}`

    const response = await fetchImpl(buildUrl(path, requestOptions.query), {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    })

    if (response.status === 204) return undefined as T
    const text = await response.text()
    // 非 JSON 响应（如网关 502 HTML/纯文本）不能让它以 SyntaxError 泄漏给调用方
    let data: unknown = null
    if (text) {
      try {
        data = JSON.parse(text)
      } catch {
        data = null
      }
    }
    if (!response.ok) {
      const problem: ProblemDetails =
        data && typeof data === 'object'
          ? (data as ProblemDetails)
          : {
              type: 'about:blank',
              title: response.statusText,
              status: response.status,
              detail: response.statusText,
              code: 'HTTP_ERROR',
            }
      throw new ApiError(problem)
    }
    return data as T
  }

  return {
    request,
    /** GET 请求 */
    get: <T>(path: string, requestOptions?: RequestOptions) => request<T>('GET', path, undefined, requestOptions),
    /** POST 请求 */
    post: <T>(path: string, body?: unknown, requestOptions?: RequestOptions) =>
      request<T>('POST', path, body, requestOptions),
    /** PUT 请求 */
    put: <T>(path: string, body?: unknown, requestOptions?: RequestOptions) =>
      request<T>('PUT', path, body, requestOptions),
    /** PATCH 请求 */
    patch: <T>(path: string, body?: unknown, requestOptions?: RequestOptions) =>
      request<T>('PATCH', path, body, requestOptions),
    /** DELETE 请求 */
    delete: <T>(path: string, requestOptions?: RequestOptions) =>
      request<T>('DELETE', path, undefined, requestOptions),
  }
}

/** API 客户端类型。 */
export type ApiClient = ReturnType<typeof createApiClient>
