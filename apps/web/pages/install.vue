<script setup lang="ts">
/** 安装引导页：iOS PWA / Android / 桌面安装说明（需求 13.5 IOS-001）。 */
const config = useRuntimeConfig()
const auth = useAuthStore()
const { platform } = useShell()
const { supported, enabled, topic, subscribeUrl, setUser, enable, disable } = usePush(
  config.public.ntfyPublicUrl,
)
const pushError = ref('')
onMounted(() => setUser(auth.user?.id ?? null))

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
</script>

<template>
  <div class="max-w-2xl space-y-4">
    <h1 class="text-xl font-semibold">安装到设备</h1>
    <p class="text-sm text-neutral-500">当前运行平台：{{ platform }}</p>

    <section class="rounded-lg border border-neutral-200 p-4 dark:border-neutral-800">
      <h2 class="text-sm font-semibold">iPhone / iPad（iOS 16.4+）</h2>
      <ol class="mt-2 list-decimal space-y-1 pl-5 text-sm text-neutral-600 dark:text-neutral-300">
        <li>使用 Safari 打开本站并登录。</li>
        <li>点击底部分享按钮，选择「添加到主屏幕」。</li>
        <li>从主屏图标启动，进入 设置 → 通知 开启推送。</li>
      </ol>
      <p class="mt-2 text-xs text-neutral-400">
        主屏 PWA 支持 Web Push（走 ntfy VAPID）；未添加到主屏时无法接收后台推送。
      </p>
    </section>

    <section class="rounded-lg border border-neutral-200 p-4 dark:border-neutral-800">
      <h2 class="text-sm font-semibold">Android / HarmonyOS</h2>
      <p class="mt-2 text-sm text-neutral-600 dark:text-neutral-300">
        请使用应用商店或内测渠道安装「社团 OA」客户端（原生壳内置推送与服务器切换）。
      </p>
    </section>

    <section class="rounded-lg border border-neutral-200 p-4 dark:border-neutral-800">
      <h2 class="text-sm font-semibold">Windows / macOS / Linux</h2>
      <p class="mt-2 text-sm text-neutral-600 dark:text-neutral-300">
        下载桌面客户端安装包（Electron），或在浏览器中使用（桌面浏览器可直接开启 Web Push）。
      </p>
    </section>

    <section class="rounded-lg border border-neutral-200 p-4 dark:border-neutral-800">
      <h2 class="text-sm font-semibold">通知</h2>
      <template v-if="supported">
        <p class="mt-1 text-sm text-neutral-500">权限：{{ enabled ? '已开启' : '未开启' }}</p>
        <Button class="mt-2" variant="secondary" @click="onTogglePush">
          {{ enabled ? '关闭通知' : '开启通知' }}
        </Button>
        <p v-if="topic" class="mt-2 break-all text-xs text-neutral-400">主题：{{ subscribeUrl }}</p>
        <p v-if="pushError" class="mt-2 text-xs text-red-500">{{ pushError }}</p>
      </template>
      <p v-else class="mt-1 text-sm text-neutral-500">当前环境不支持通知。</p>
    </section>
  </div>
</template>
