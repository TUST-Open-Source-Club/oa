<script setup lang="ts">
/** 公开分享页：展示文件名与大小，可下载；带密码分享需先验证。 */
definePageMeta({ layout: 'auth' })

interface ShareInfo {
  passwordProtected: boolean
  name: string
  kind: string
  size: number
  mime: string | null
  permission: string
  maxDownloads: number
  downloadCount: number
}

const route = useRoute()
const token = computed(() => String(route.params.token ?? ''))
const info = ref<ShareInfo | null>(null)
const password = ref('')
const needPassword = ref(false)
const errorMessage = ref('')
const loading = ref(false)

/** 加载分享信息（带密码时重试）。 */
async function load(withPassword = false) {
  loading.value = true
  errorMessage.value = ''
  try {
    info.value = await $fetch<ShareInfo>(`/api/share/${token.value}`, {
      params: withPassword && password.value ? { password: password.value } : {},
    })
    needPassword.value = false
  } catch (error) {
    const status = (error as { statusCode?: number }).statusCode
    if (status === 401 || status === 403) {
      needPassword.value = true
      if (withPassword) errorMessage.value = '密码错误，请重试'
    } else {
      errorMessage.value = apiErrorMessage(error, '分享不存在或已失效')
    }
  } finally {
    loading.value = false
  }
}

/** 下载文件。 */
async function download() {
  if (!info.value) return
  errorMessage.value = ''
  try {
    const query = password.value ? `?password=${encodeURIComponent(password.value)}` : ''
    const response = await fetch(`/api/share/${token.value}/download${query}`)
    if (!response.ok) {
      errorMessage.value = response.status === 403 ? '下载次数已用完' : '下载失败'
      return
    }
    const blob = await response.blob()
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = info.value.name
    link.click()
    URL.revokeObjectURL(url)
    await load(!!password.value)
  } catch {
    errorMessage.value = '下载失败'
  }
}

/** 大小展示。 */
function formatSize(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`
  return `${(size / 1024 / 1024 / 1024).toFixed(2)} GB`
}

onMounted(() => load())
</script>

<template>
  <div class="w-full max-w-md">
    <div class="rounded-[var(--radius-card)] border border-neutral-200 bg-white p-6 shadow-[var(--shadow-card)] dark:border-neutral-800 dark:bg-neutral-900">
      <div class="mb-4 flex items-center gap-3">
        <span class="flex size-11 items-center justify-center rounded-xl bg-primary-50 text-primary-600 dark:bg-primary-900/40">
          <AppIcon name="folder" class="size-6" />
        </span>
        <div class="min-w-0">
          <h1 class="truncate text-base font-semibold">{{ info?.name ?? '文件分享' }}</h1>
          <p v-if="info" class="text-xs text-neutral-400">
            {{ formatSize(info.size) }} · 已下载 {{ info.downloadCount }} 次
            <span v-if="info.maxDownloads > 0"> / 限 {{ info.maxDownloads }} 次</span>
          </p>
        </div>
      </div>

      <p v-if="errorMessage" class="mb-3 text-sm text-danger-500">{{ errorMessage }}</p>

      <form v-if="needPassword" class="space-y-3" @submit.prevent="load(true)">
        <input
          v-model="password"
          type="password"
          placeholder="请输入访问密码"
          class="h-10 w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 text-sm outline-none focus:border-primary-500 dark:border-neutral-700"
        />
        <button
          type="submit"
          class="h-10 w-full rounded-[var(--radius-field)] bg-primary-600 text-sm font-medium text-white transition hover:bg-primary-700 disabled:opacity-60"
          :disabled="loading"
        >
          {{ loading ? '验证中…' : '验证并查看' }}
        </button>
      </form>

      <button
        v-else-if="info"
        class="h-10 w-full rounded-[var(--radius-field)] bg-primary-600 text-sm font-medium text-white transition hover:bg-primary-700"
        @click="download"
      >
        下载文件
      </button>
    </div>
    <p class="mt-4 text-center text-xs text-neutral-400">
      {{ $config.public.appName }} · 文件分享
    </p>
  </div>
</template>
