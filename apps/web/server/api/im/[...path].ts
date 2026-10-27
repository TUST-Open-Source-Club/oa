/** IM 服务 JSON 代理。 */
export default defineEventHandler((event) =>
  proxyServiceJson(event, useRuntimeConfig(event).imServiceUrl, '/api/v1/im', 'IM_SERVICE_ERROR'),
)
