<script setup lang="ts">
/** 设置页：服务器信息与切换、登录设备、通知开关、客户端平台、安装引导（需求 13.1.1）。 */
import { Button } from '@club-oa/ui'

const config = useRuntimeConfig()
const auth = useAuthStore()
const { platform, isShell, vendor, switchServer } = useShell()
const { supported, enabled, permission, topic, subscribeUrl, setUser, enable, disable } =
  usePush(config.public.ntfyPublicUrl)

const origin = ref('')
const pushError = ref('')
const switching = ref(false)

/** 登录设备。 */
interface DeviceSession {
  familyId: string
  deviceId: string | null
  userAgent: string | null
  ip: string | null
  createdAt: string
  expiresAt: string
  current: boolean
}
const sessions = ref<DeviceSession[]>([])
const sessionsLoading = ref(false)
const sessionError = ref('')

/** 设备友好名称（无 deviceId 时按 UA 猜）。 */
function deviceLabel(row: DeviceSession) {
  if (row.deviceId) return row.deviceId
  const ua = row.userAgent ?? ''
  if (/iPhone|iPad|iOS/.test(ua)) return 'iOS 设备'
  if (/Android/.test(ua)) return 'Android 设备'
  if (/Macintosh|Mac OS/.test(ua)) return 'Mac'
  if (/Windows/.test(ua)) return 'Windows'
  if (/Linux/.test(ua)) return 'Linux'
  return '未知设备'
}

/** 登录时间显示。 */
function sessionTime(value: string) {
  return new Date(value).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** 拉取登录设备。 */
async function loadSessions() {
  sessionsLoading.value = true
  sessionError.value = ''
  try {
    sessions.value = await $fetch<DeviceSession[]>('/api/auth/sessions', {
      headers: auth.authHeaders(),
    })
  } catch (err) {
    sessionError.value = (err as Error).message
  } finally {
    sessionsLoading.value = false
  }
}

/** 登出某台设备；若登出的是当前设备，直接回到登录页。 */
async function revokeSession(row: DeviceSession) {
  sessionError.value = ''
  try {
    await $fetch(`/api/auth/sessions/${row.familyId}`, {
      method: 'DELETE',
      headers: auth.authHeaders(),
    })
    if (row.current) {
      await auth.logout()
      await navigateTo('/login')
      return
    }
    await loadSessions()
  } catch (err) {
    sessionError.value = (err as Error).message
  }
}

onMounted(() => {
  origin.value = window.location.origin
  setUser(auth.user?.id ?? null)
  void loadSessions()
})
watch(
  () => auth.user?.id,
  (id) => setUser(id ?? null),
)

/** 切换通知开关。 */
async function onTogglePush() {
  pushError.value = ''
  try {
    if (enabled.value) disable()
    else await enable()
  } catch (error) {
    pushError.value = (error as Error).message
  }
}

/** 壳内切换服务器（清除配置并回到壳的设置页）。 */
async function onSwitchServer() {
  switching.value = true
  try {
    await switchServer()
  } finally {
    switching.value = false
  }
}
</script>

<template>
  <div class="mx-auto w-full max-w-[720px] space-y-6">
    <header>
      <p class="text-[12px] text-neutral-500">服务器、通知与客户端偏好</p>
    </header>

    <!-- 服务器 -->
    <section
      class="overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <div class="border-b border-[var(--border)] px-4 py-2.5">
        <h2 class="text-[12px] font-medium text-neutral-500">服务器</h2>
      </div>
      <dl>
        <div class="flex items-center justify-between gap-4 px-4 py-2.5">
          <dt class="shrink-0 text-[12px] text-neutral-500">当前地址</dt>
          <dd class="truncate text-[12px] text-[var(--text-strong)]">{{ origin || '—' }}</dd>
        </div>
        <div class="flex items-center justify-between gap-4 border-t border-[var(--border)] px-4 py-2.5">
          <dt class="shrink-0 text-[12px] text-neutral-500">客户端</dt>
          <dd class="text-[12px] text-[var(--text-strong)]">{{ platform }}</dd>
        </div>
        <div
          v-if="vendor"
          class="flex items-center justify-between gap-4 border-t border-[var(--border)] px-4 py-2.5"
        >
          <dt class="shrink-0 text-[12px] text-neutral-500">推送厂商</dt>
          <dd class="text-[12px] text-[var(--text-strong)]">{{ vendor }}</dd>
        </div>
      </dl>
      <div v-if="isShell" class="border-t border-[var(--border)] px-4 py-3">
        <Button variant="secondary" size="sm" :disabled="switching" @click="onSwitchServer">
          {{ switching ? '正在打开设置页…' : '切换服务器' }}
        </Button>
      </div>
    </section>

    <!-- 登录设备 -->
    <section
      class="overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <div class="flex items-center justify-between border-b border-[var(--border)] px-4 py-2.5">
        <h2 class="text-[12px] font-medium text-neutral-500">登录设备</h2>
        <button
          type="button"
          class="text-[11px] text-neutral-400 transition-colors hover:text-primary-600 disabled:opacity-50"
          :disabled="sessionsLoading"
          @click="loadSessions"
        >
          刷新
        </button>
      </div>
      <p v-if="sessionsLoading" class="px-4 py-6 text-center text-[12px] text-neutral-400">
        加载中…
      </p>
      <p
        v-else-if="sessions.length === 0"
        class="px-4 py-6 text-center text-[12px] text-neutral-400"
      >
        暂无活跃设备
      </p>
      <ul v-else class="divide-y divide-[var(--border)]">
        <li
          v-for="row in sessions"
          :key="row.familyId"
          class="flex items-center justify-between gap-4 px-4 py-2.5"
        >
          <div class="min-w-0">
            <p class="flex items-center gap-1.5 text-[12px] text-[var(--text-strong)]">
              <span class="truncate">{{ deviceLabel(row) }}</span>
              <span
                v-if="row.current"
                class="shrink-0 rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40 dark:text-primary-200"
              >当前设备</span>
            </p>
            <p class="mt-0.5 truncate text-[11px] text-neutral-400">
              登录于 {{ sessionTime(row.createdAt) }}
              <template v-if="row.ip"> · {{ row.ip }}</template>
            </p>
          </div>
          <Button variant="secondary" size="sm" @click="revokeSession(row)">登出</Button>
        </li>
      </ul>
      <p v-if="sessionError" class="px-4 pb-3 text-[11px] text-danger-500">{{ sessionError }}</p>
    </section>

    <!-- 通知 -->
    <section
      class="overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <div class="border-b border-[var(--border)] px-4 py-2.5">
        <h2 class="text-[12px] font-medium text-neutral-500">通知</h2>
      </div>
      <div class="px-4 py-3">
        <p v-if="!supported" class="text-[12px] text-neutral-500">当前环境不支持通知。</p>
        <template v-else>
          <div class="flex items-center justify-between gap-4">
            <p class="text-[12px] text-neutral-500">通知权限：{{ permission }}</p>
            <Button variant="secondary" size="sm" @click="onTogglePush">
              {{ enabled ? '关闭通知' : '开启通知' }}
            </Button>
          </div>
          <p v-if="topic" class="mt-2 truncate text-[11px] text-neutral-400">ntfy 主题：{{ topic }}</p>
          <p v-if="subscribeUrl" class="mt-1 break-all text-[11px] text-neutral-400">
            订阅地址：{{ subscribeUrl }}
          </p>
          <p v-if="pushError" class="mt-2 text-[11px] text-danger-500">{{ pushError }}</p>
        </template>
      </div>
    </section>

    <!-- 客户端 -->
    <section
      class="overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <div class="border-b border-[var(--border)] px-4 py-2.5">
        <h2 class="text-[12px] font-medium text-neutral-500">客户端</h2>
      </div>
      <NuxtLink
        to="/install"
        class="flex items-center justify-between gap-4 px-4 py-3 text-[12px] text-[var(--text-strong)] transition-colors hover:bg-[var(--surface-hover)]"
      >
        <span>安装到主屏 / 桌面（iOS PWA、Android、桌面）</span>
        <AppIcon name="arrowRight" class="size-4 shrink-0 text-neutral-300" />
      </NuxtLink>
    </section>
  </div>
</template>
