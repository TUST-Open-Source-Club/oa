<script setup lang="ts">
/** 全局错误页：壳内提供「切换服务器」入口（需求 DESK-014 / AND-010 / HARM-009）。 */
import type { NuxtError } from '#app'

const props = defineProps<{ error: NuxtError }>()
const { isShell, switchServer } = useShell()
const switching = ref(false)

/** 返回首页。 */
function goHome() {
  clearError({ redirect: '/' })
}

/** 壳内切换服务器：清除配置并回到壳的设置页，随后回到首页。 */
async function onSwitchServer() {
  switching.value = true
  try {
    await switchServer()
    clearError({ redirect: '/' })
  } finally {
    switching.value = false
  }
}
</script>

<template>
  <div class="flex min-h-screen flex-col items-center justify-center gap-4 p-8">
    <h1 class="text-2xl font-semibold">{{ props.error.statusCode }}</h1>
    <p class="text-sm text-neutral-500">{{ props.error.message || '页面出错了' }}</p>
    <div class="flex gap-3">
      <button class="rounded bg-primary-600 px-4 py-2 text-sm text-white" @click="goHome">
        返回首页
      </button>
      <button
        v-if="isShell"
        class="rounded border border-neutral-300 px-4 py-2 text-sm"
        :disabled="switching"
        @click="onSwitchServer"
      >
        {{ switching ? '正在打开设置页…' : '切换服务器' }}
      </button>
    </div>
  </div>
</template>
