import { app } from 'electron'
import { promises as fs } from 'node:fs'
import { join } from 'node:path'

/**
 * 服务器地址配置（见需求 13.1.1）：不硬编码，首次启动校验 OIDC discovery 后保存。
 */
interface ConfigFile {
  baseUrl?: string
  issuer?: string
}

function configPath(): string {
  return join(app.getPath('userData'), 'config.json')
}

/** 读取配置。 */
export async function loadConfig(): Promise<ConfigFile> {
  try {
    return JSON.parse(await fs.readFile(configPath(), 'utf8')) as ConfigFile
  } catch {
    return {}
  }
}

/** 保存服务器地址。 */
export async function saveServer(baseUrl: string, issuer: string): Promise<void> {
  const normalized = baseUrl.trim().replace(/\/+$/, '')
  await fs.writeFile(configPath(), JSON.stringify({ baseUrl: normalized, issuer }, null, 2), 'utf8')
}

/** 清除服务器配置（切换服务器时调用）。 */
export async function clearServer(): Promise<void> {
  await fs.writeFile(configPath(), JSON.stringify({}, null, 2), 'utf8')
}

/**
 * 校验服务器：请求 `/.well-known/openid-configuration`，返回 issuer。
 * 仅接受 HTTPS（本地联调允许 http://localhost 与 http://127.0.0.1）。
 */
export async function validateServer(baseUrl: string): Promise<string> {
  const normalized = baseUrl.trim().replace(/\/+$/, '')
  const allowed =
    normalized.startsWith('https://') ||
    normalized.startsWith('http://localhost') ||
    normalized.startsWith('http://127.0.0.1')
  if (!allowed) {
    throw new Error('仅支持 HTTPS 服务器地址')
  }
  const response = await fetch(`${normalized}/.well-known/openid-configuration`)
  if (!response.ok) {
    throw new Error(`服务器不可达（HTTP ${response.status}）`)
  }
  const body = (await response.json()) as { issuer?: string }
  if (!body.issuer) {
    throw new Error('服务器未返回合法的 OIDC issuer')
  }
  return body.issuer
}
