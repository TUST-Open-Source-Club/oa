<script setup lang="ts">
/** 设置页：服务器信息与切换、通知开关、客户端平台、安装引导（需求 13.1.1）。 */
import { Button, Card } from '@club-oa/ui'

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
  <div class="max-w-2xl space-y-4">
    <h1 class="text-xl font-semibold">设置</h1>

    <Card>
      <h2 class="mb-2 text-sm font-semibold text-neutral-700 dark:text-neutral-200">服务器</h2>
      <p class="text-sm text-neutral-500">当前地址：{{ origin || '—' }}</p>
      <p class="mt-1 text-sm text-neutral-500">客户端：{{ platform }}</p>
      <p v-if="vendor" class="mt-1 text-sm text-neutral-500">推送厂商：{{ vendor }}</p>
      <Button v-if="isShell" class="mt-3" variant="secondary" :disabled="switching" @click="onSwitchServer">
        {{ switching ? '正在打开设置页…' : '切换服务器' }}
      </Button>
    </Card>

    <Card>
      <h2 class="mb-2 text-sm font-semibold text-neutral-700 dark:text-neutral-200">通知</h2>
      <p v-if="!supported" class="text-sm text-neutral-500">当前环境不支持通知。</p>
      <template v-else>
        <p class="text-sm text-neutral-500">通知权限：{{ permission }}</p>
        <p v-if="topic" class="mt-1 break-all text-xs text-neutral-400">ntfy 主题：{{ topic }}</p>
        <p v-if="subscribeUrl" class="mt-1 break-all text-xs text-neutral-400">订阅地址：{{ subscribeUrl }}</p>
        <Button class="mt-3" variant="secondary" @click="onTogglePush">
          {{ enabled ? '关闭通知' : '开启通知' }}
        </Button>
        <p v-if="pushError" class="mt-2 text-xs text-red-500">{{ pushError }}</p>
      </template>
    </Card>

    <Card>
      <h2 class="mb-2 text-sm font-semibold text-neutral-700 dark:text-neutral-200">客户端</h2>
      <NuxtLink to="/install" class="text-sm text-primary-600 hover:underline">
        安装到主屏 / 桌面（iOS PWA、Android、桌面）
      </NuxtLink>
    </Card>
  </div>
</template>
