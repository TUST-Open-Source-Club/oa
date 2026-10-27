/** task 服务 JSON 代理。 */
export default defineEventHandler((event) =>
  proxyServiceJson(event, useRuntimeConfig(event).taskServiceUrl, '/api/v1/task', 'TASK_SERVICE_ERROR'),
)
