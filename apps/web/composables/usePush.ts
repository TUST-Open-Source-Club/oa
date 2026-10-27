import { computed, ref } from 'vue'

/**
 * 通知许可与 ntfy 主题（见需求 13.5 IOS-004、13.7）：
 * - Web Push 订阅由 ntfy 服务端承载（VAPID），主题为用户级不可猜测串
 * - 前台实时消息由 ntfy WebSocket 推送（后续接入）
 */

/** 计算用户 ntfy 主题（与 notify 服务 domain::topic_for 对齐）。 */
export function topicFor(userId: string): string {
  return `u_${userId.replace(/-/g, '')}`
}

/** 通知能力封装。 */
export function usePush(ntfyUrl = '') {
  const win = typeof window === 'undefined' ? undefined : window
  const supported = !!win && 'Notification' in win
  const permission = ref<NotificationPermission>(
    supported ? win.Notification.permission : 'default',
  )
  const userId = ref<string | null>(null)

  /** 是否可收到通知（已授权）。 */
  const enabled = computed(() => supported && permission.value === 'granted')
  /** 当前用户 ntfy 主题。 */
  const topic = computed(() => (userId.value ? topicFor(userId.value) : null))
  /** ntfy 公共地址（订阅用）。 */
  const subscribeUrl = computed(() => (topic.value ? `${ntfyUrl}/${topic.value}` : null))

  /** 记录当前用户（登录后调用）。 */
  function setUser(id: string | null): void {
    userId.value = id
  }

  /** 请求通知权限。 */
  async function enable(): Promise<void> {
    if (!supported) throw new Error('当前环境不支持通知')
    permission.value = await win.Notification.requestPermission()
    if (permission.value !== 'granted') throw new Error('未获得通知权限')
  }

  /** 停用通知（仅本地状态，服务端偏好另行关闭）。 */
  function disable(): void {
    permission.value = 'default'
  }

  return { supported, permission, enabled, topic, subscribeUrl, setUser, enable, disable }
}
