/** BFF 激活账号：代理 auth 服务的 /users/activate。 */
export default defineEventHandler(async (event) => {
  const body = await readBody<{ token: string; password: string }>(event)
  await callAuthService(event, '/api/v1/auth/users/activate', body)
  return { ok: true }
})
