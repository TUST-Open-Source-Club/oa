/**
 * 认证状态：Access Token 仅存内存；Refresh Token 由 BFF 放在 httpOnly Cookie。
 * 页面刷新后通过 `restore()` 调用 BFF 换发新的 Access Token。
 */
// 显式导入 $fetch（而非依赖 Nuxt 自动导入），保证 store 可在 Vitest 中独立测试
import { $fetch } from 'ofetch'
import { defineStore } from 'pinia'

/** 对外用户信息（与 auth 服务 UserDto 对齐）。 */
export interface AuthUser {
  id: string
  username: string
  email: string
  nickname: string
  avatar?: string | null
  roles: string[]
  status: string
}

interface LoginResponse {
  accessToken: string
  expiresIn: number
  user: AuthUser
}

interface RefreshResponse {
  accessToken: string
  expiresIn: number
}

export const useAuthStore = defineStore('auth', {
  state: () => ({
    accessToken: '' as string,
    user: null as AuthUser | null,
    /** 是否已完成一次会话恢复尝试（供路由守卫判断） */
    ready: false,
  }),

  getters: {
    /** 是否已登录。 */
    isAuthenticated: (state) => Boolean(state.accessToken && state.user),
    /** 是否拥有管理员角色。 */
    isAdmin: (state) =>
      Boolean(state.user?.roles.some((role) => role === 'admin' || role === 'superadmin')),
  },

  actions: {
    /** 密码登录（凭 BFF 写入 httpOnly 刷新 Cookie）。 */
    async login(identifier: string, password: string) {
      const response = await $fetch<LoginResponse>('/api/auth/login', {
        method: 'POST',
        body: { identifier, password },
      })
      this.accessToken = response.accessToken
      this.user = response.user
      this.ready = true
    },

    /** 页面刷新后恢复会话；无有效刷新 Cookie 时静默失败。 */
    async restore() {
      try {
        const response = await $fetch<RefreshResponse>('/api/auth/refresh', { method: 'POST' })
        this.accessToken = response.accessToken
        this.user = await $fetch<AuthUser>('/api/auth/me', {
          headers: { authorization: `Bearer ${response.accessToken}` },
        })
      } catch {
        this.clear()
      } finally {
        this.ready = true
      }
    },

    /** 退出登录：吊销刷新令牌并清空本地状态（服务端失败不阻断本地退出）。 */
    async logout() {
      try {
        await $fetch('/api/auth/logout', { method: 'POST' })
      } catch {
        // 忽略吊销失败：刷新令牌会自然过期，本地会话必须清理
      }
      this.clear()
    },

    /** 清空本地会话。 */
    clear() {
      this.accessToken = ''
      this.user = null
    },

    /** 生成调用业务服务的鉴权头。 */
    authHeaders(): Record<string, string> {
      return this.accessToken ? { authorization: `Bearer ${this.accessToken}` } : {}
    },
  },
})
