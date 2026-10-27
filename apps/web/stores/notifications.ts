/**
 * 通知中心状态：列表、未读数、已读操作。
 * 所有请求经 BFF（/api/notify/*），携带内存中的 Access Token。
 */
import { $fetch } from 'ofetch'
import { defineStore } from 'pinia'

import { useAuthStore } from './auth'

/** 通知项（与 notify 服务 NotificationDto 对齐）。 */
export interface NotificationItem {
  id: string
  eventType: string
  title: string
  body: string
  priority: string
  url?: string | null
  read: boolean
  createdAt: string
}

interface ListResponse {
  items: NotificationItem[]
  nextCursor: string | null
}

export const useNotificationsStore = defineStore('notifications', {
  state: () => ({
    items: [] as NotificationItem[],
    unreadCount: 0,
    nextCursor: null as string | null,
    loading: false,
  }),

  getters: {
    /** 是否存在未读通知（角标用）。 */
    hasUnread: (state) => state.unreadCount > 0,
  },

  actions: {
    /** 拉取未读数。 */
    async fetchUnreadCount() {
      const auth = useAuthStore()
      const response = await $fetch<{ count: number }>('/api/notify/notifications/unread-count', {
        headers: auth.authHeaders(),
      })
      this.unreadCount = response.count
    },

    /** 拉取列表（append=true 时按 nextCursor 加载更多）。 */
    async fetchList(options: { unreadOnly?: boolean; append?: boolean } = {}) {
      const auth = useAuthStore()
      this.loading = true
      try {
        const query: Record<string, string> = {}
        if (options.unreadOnly) query.unreadOnly = 'true'
        if (options.append && this.nextCursor) query.cursor = this.nextCursor
        const response = await $fetch<ListResponse>('/api/notify/notifications', {
          query,
          headers: auth.authHeaders(),
        })
        this.items = options.append ? [...this.items, ...response.items] : response.items
        this.nextCursor = response.nextCursor
      } finally {
        this.loading = false
      }
    },

    /** 标记单条已读（本地同步递减未读数）。 */
    async markRead(id: string) {
      const auth = useAuthStore()
      await $fetch(`/api/notify/notifications/${id}/read`, {
        method: 'POST',
        headers: auth.authHeaders(),
      })
      const item = this.items.find((entry) => entry.id === id)
      if (item && !item.read) {
        item.read = true
        this.unreadCount = Math.max(0, this.unreadCount - 1)
      }
    },

    /** 全部已读。 */
    async markAllRead() {
      const auth = useAuthStore()
      await $fetch('/api/notify/notifications/read-all', {
        method: 'POST',
        headers: auth.authHeaders(),
      })
      this.items.forEach((item) => {
        item.read = true
      })
      this.unreadCount = 0
    },
  },
})
