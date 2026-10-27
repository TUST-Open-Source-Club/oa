/** event 服务 JSON 代理。 */
export default defineEventHandler((event) =>
  proxyServiceJson(event, useRuntimeConfig(event).eventServiceUrl, '/api/v1/event', 'EVENT_SERVICE_ERROR'),
)
