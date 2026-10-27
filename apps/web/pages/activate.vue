<script setup lang="ts">
/** 账号激活页：凭邮件中的一次性 token 设置密码。 */
import { Button, Card, Input } from '@club-oa/ui'
import { ApiError } from '@club-oa/core'

definePageMeta({ layout: 'auth' })

const route = useRoute()
const router = useRouter()
const token = computed(() => (typeof route.query.token === 'string' ? route.query.token : ''))

const password = ref('')
const confirm = ref('')
const loading = ref(false)
const errorMessage = ref('')

/** 提交激活：校验两次密码一致后调用 auth 服务。 */
async function onSubmit() {
  if (!token.value) {
    errorMessage.value = '激活链接缺少 token，请检查邮件'
    return
  }
  if (password.value !== confirm.value) {
    errorMessage.value = '两次输入的密码不一致'
    return
  }
  loading.value = true
  errorMessage.value = ''
  try {
    await $fetch('/api/auth/activate', {
      method: 'POST',
      body: { token: token.value, password: password.value },
    })
    await router.push({ path: '/login', query: { activated: '1' } })
  } catch (error) {
    errorMessage.value = error instanceof ApiError ? error.message : '网络异常，请稍后重试'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <Card>
    <h2 class="mb-1 text-lg font-semibold">激活账号</h2>
    <p class="mb-6 text-sm text-neutral-500">设置密码后即可登录。</p>
    <form class="space-y-4" @submit.prevent="onSubmit">
      <Input v-model="password" label="新密码" type="password" autocomplete="new-password" />
      <Input v-model="confirm" label="确认密码" type="password" autocomplete="new-password" />
      <p v-if="errorMessage" role="alert" class="text-sm text-danger-500">{{ errorMessage }}</p>
      <Button type="submit" block :loading="loading">设置密码并激活</Button>
    </form>
  </Card>
</template>
