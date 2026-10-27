import tailwindcss from '@tailwindcss/vite'

/**
 * Web 门户（Nuxt 3）：SSR 壳 + BFF。
 * - BFF 持有 refresh token（httpOnly Cookie），Access Token 仅存客户端内存
 * - 设计 token 来自 @club-oa/ui（与 Tauri 客户端共用）
 */
export default defineNuxtConfig({
  compatibilityDate: '2026-09-13',
  devtools: { enabled: false },
  app: {
    head: {
      link: [{ rel: 'manifest', href: '/manifest.webmanifest' }],
      meta: [{ name: 'theme-color', content: '#4f46e5' }],
    },
  },
  modules: ['@pinia/nuxt'],
  css: ['~/assets/css/main.css'],
  vite: {
    plugins: [tailwindcss()],
  },
  runtimeConfig: {
    // 服务端专用：内部服务地址（不进浏览器）
    authServiceUrl: process.env.AUTH_SERVICE_URL || 'http://localhost:8081',
    notifyServiceUrl: process.env.NOTIFY_SERVICE_URL || 'http://localhost:8088',
    driveServiceUrl: process.env.DRIVE_SERVICE_URL || 'http://localhost:8087',
    docServiceUrl: process.env.DOC_SERVICE_URL || 'http://localhost:8084',
    taskServiceUrl: process.env.TASK_SERVICE_URL || 'http://localhost:8083',
    eventServiceUrl: process.env.EVENT_SERVICE_URL || 'http://localhost:8086',
    imServiceUrl: process.env.IM_SERVICE_URL || 'http://localhost:8082',
    public: {
      appName: '社团 OA',
      // 前端订阅 ntfy（Web Push / WebSocket）用地址
      ntfyPublicUrl: process.env.NTFY_PUBLIC_URL || 'http://localhost/ntfy',
    },
  },
  typescript: { strict: true },
})
