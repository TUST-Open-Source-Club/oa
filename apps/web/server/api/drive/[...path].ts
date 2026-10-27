/** drive 服务 JSON 代理。 */
export default defineEventHandler((event) =>
  proxyServiceJson(event, useRuntimeConfig(event).driveServiceUrl, '/api/v1/drive', 'DRIVE_SERVICE_ERROR'),
)
