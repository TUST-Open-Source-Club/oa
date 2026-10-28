<script setup lang="ts">
/** 文档公开分享页：免登录只读，带密码时先校验。 */
definePageMeta({ layout: 'auth' })

interface PublicDoc {
  title: string
  contentMd: string
  updatedAt: string
  spaceName: string | null
  permission: string
}

const route = useRoute()
const token = computed(() => String(route.params.token ?? ''))
const doc = ref<PublicDoc | null>(null)
const password = ref('')
const needPassword = ref(false)
const errorMessage = ref('')
const loading = ref(false)

/** 读取分享文档（带密码时重试）。 */
async function load(withPassword = false) {
  loading.value = true
  errorMessage.value = ''
  try {
    doc.value = await $fetch<PublicDoc>(`/api/doc/public/${token.value}`, {
      params: withPassword && password.value ? { password: password.value } : {},
    })
    needPassword.value = false
  } catch (error) {
    const status = (error as { statusCode?: number }).statusCode
    if (status === 401) {
      needPassword.value = true
      if (withPassword) errorMessage.value = '密码错误，请重试'
    } else if (status === 410) {
      errorMessage.value = '分享链接已过期'
    } else {
      errorMessage.value = apiErrorMessage(error, '分享不存在或已关闭')
    }
  } finally {
    loading.value = false
  }
}

onMounted(() => void load())
</script>

<template>
  <div class="mx-auto w-full max-w-[820px] px-6 py-10">
    <p v-if="loading" class="py-16 text-center text-[13px] text-neutral-400">加载中…</p>

    <template v-else-if="doc">
      <header class="mb-6 border-b border-[var(--border)] pb-4">
        <p class="text-[11px] text-neutral-400">
          {{ doc.spaceName ?? '文档' }} · 最后更新
          {{ new Date(doc.updatedAt).toLocaleString('zh-CN') }}
        </p>
        <h1 class="mt-1 text-[28px] leading-tight font-semibold tracking-[-0.02em] text-[var(--text-strong)]">
          {{ doc.title }}
        </h1>
      </header>
      <article class="min-h-[40vh]">
        <MarkdownEditor :model-value="doc.contentMd" readonly />
      </article>
      <footer class="mt-10 border-t border-[var(--border)] pt-3 text-[11px] text-neutral-400">
        本页为只读分享，内容以原始文档为准。
      </footer>
    </template>

    <div
      v-else-if="needPassword"
      class="mx-auto mt-16 w-full max-w-sm rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] p-5 shadow-[var(--shadow-card)]"
    >
      <h2 class="text-[15px] font-medium text-[var(--text-strong)]">该分享已加密</h2>
      <p class="mt-1 text-[12px] text-neutral-500">请输入访问密码继续查看。</p>
      <input
        v-model="password"
        type="password"
        placeholder="访问密码"
        class="mt-3 h-9 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
        @keydown.enter.prevent="load(true)"
      />
      <p v-if="errorMessage" class="mt-2 text-[11px] text-danger-500">{{ errorMessage }}</p>
      <Button class="mt-3 w-full" :disabled="!password.trim()" @click="load(true)">查看</Button>
    </div>

    <div v-else class="mx-auto mt-16 max-w-sm text-center">
      <p class="text-[13px] text-neutral-500">{{ errorMessage || '分享不存在或已关闭' }}</p>
    </div>
  </div>
</template>
