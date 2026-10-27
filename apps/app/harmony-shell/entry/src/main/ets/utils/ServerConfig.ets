import preferences from '@ohos.data.preferences';
import http from '@ohos.net.http';

/**
 * 服务器地址配置（见需求 13.1.1）：不硬编码，首次启动校验 OIDC discovery 后保存。
 */
export class ServerConfig {
  private static readonly STORE = 'club_oa';
  private static readonly KEY = 'base_url';

  /** 读取服务器地址（未配置返回空串）。 */
  static async get(context: Context): Promise<string> {
    const store = await preferences.getPreferences(context, ServerConfig.STORE);
    return (await store.get(ServerConfig.KEY, '')) as string;
  }

  /** 保存服务器地址。 */
  static async set(context: Context, baseUrl: string): Promise<void> {
    const store = await preferences.getPreferences(context, ServerConfig.STORE);
    await store.put(ServerConfig.KEY, ServerConfig.normalize(baseUrl));
    await store.flush();
  }

  /** 清除服务器配置（切换服务器时调用）。 */
  static async clear(context: Context): Promise<void> {
    const store = await preferences.getPreferences(context, ServerConfig.STORE);
    await store.clear();
    await store.flush();
  }

  /** 归一化：去尾部斜杠。 */
  static normalize(baseUrl: string): string {
    return baseUrl.trim().replace(/\/+$/, '');
  }

  /**
   * 校验服务器：请求 `/.well-known/openid-configuration`，返回 issuer。
   * 仅接受 HTTPS（本地联调允许 http://10.0.2.2）。
   */
  static async validate(baseUrl: string): Promise<string> {
    const normalized = ServerConfig.normalize(baseUrl);
    if (!normalized.startsWith('https://') && !normalized.startsWith('http://10.0.2.2')) {
      throw new Error('仅支持 HTTPS 服务器地址');
    }
    const request = http.createHttp();
    try {
      const response = await request.request(`${normalized}/.well-known/openid-configuration`, {
        method: http.RequestMethod.GET,
        connectTimeout: 8000,
        readTimeout: 8000
      });
      const body = JSON.parse(response.result as string) as Record<string, string>;
      if (!body.issuer) {
        throw new Error('服务器未返回合法的 OIDC issuer');
      }
      return body.issuer;
    } finally {
      request.destroy();
    }
  }
}
