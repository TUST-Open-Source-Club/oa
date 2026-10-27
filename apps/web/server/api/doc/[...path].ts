/** doc 服务 JSON 代理。 */
export default defineEventHandler((event) =>
  proxyServiceJson(event, useRuntimeConfig(event).docServiceUrl, '/api/v1/doc', 'DOC_SERVICE_ERROR'),
)
