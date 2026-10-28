<script setup lang="ts">
/** 设置页：服务器信息与切换、通知开关、客户端平台、安装引导（需求 13.1.1）。 */
import { Button } from '@club-oa/ui'

const config = useRuntimeConfig()
const auth = useAuthStore()
const { platform, isShell, vendor, switchServer } = useShell()
const { supported, enabled, permission, topic, subscribeUrl, setUser, enable, disable } =
  usePush(config.public.ntfyPublicUrl)

const origin = ref('')
const pushError = ref('')
const switching = ref(false)

onMounted(() => {
  origin.value = window.location.origin
  setUser(auth.user?.id ?? null)
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
