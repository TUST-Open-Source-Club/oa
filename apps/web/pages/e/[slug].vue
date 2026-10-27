<script setup lang="ts">
/** 公开活动报名页：动态表单、邮箱验证码、提交与状态查询（无需登录）。 */
import { Button, Card, Input } from '@club-oa/ui'

interface FormField {
  key: string
  type: string
  label: string
  required?: boolean
  options?: Array<{ label: string; value: string }>
}
interface PublicEvent {
  slug: string
  title: string
  descriptionMd: string
  location: string | null
  capacity: number
  remaining: number | null
  waitlistEnabled: boolean
  needReview: boolean
  emailVerify: boolean
  form: { fields: FormField[] }
}

definePageMeta({ layout: 'auth' })

const route = useRoute()
const slug = String(route.params.slug)
const event = ref<PublicEvent | null>(null)
const answers = reactive<Record<string, unknown>>({})
const code = ref('')
const devCode = ref('')
const sending = ref(false)
const submitting = ref(false)
const errorMessage = ref('')
const result = ref<{ status: string; checkinCode: string } | null>(null)

/** 查询状态。 */
const query = reactive({ code: '', email: '' })
const queryResult = ref<{ statusLabel: string; checkedIn: boolean } | null>(null)
const queryError = ref('')

/** 初始化默认值。 */
function defaultFor(field: FormField): unknown {
  if (field.type === 'checkbox') return false
  if (field.type === 'multiselect') return []
  return ''
}

onMounted(async () => {
  try {
    event.value = await $fetch<PublicEvent>(`/api/event/public/events/${slug}`)
    for (const field of event.value.form.fields) {
      answers[field.key] = defaultFor(field)
    }
  } catch {
    errorMessage.value = '活动不存在或未开放报名'
  }
})

/** 发送邮箱验证码（以表单 email 字段为准）。 */
async function sendCode() {
  const email = String(answers.email ?? '').trim()
  if (!email) {
    errorMessage.value = '请先填写邮箱'
    return
  }
  sending.value = true
  errorMessage.value = ''
  try {
    const response = await $fetch<{ devCode?: string }>(
      `/api/event/public/events/${slug}/send-code`,
      { method: 'POST', body: { email } },
    )
    devCode.value = response.devCode ?? ''
  } catch (error) {
    const data = (error as { data?: { detail?: string } }).data
    errorMessage.value = data?.detail ?? '验证码发送失败'
  } finally {
    sending.value = false
  }
}

/** 提交报名。 */
async function submit() {
  submitting.value = true
  errorMessage.value = ''
  try {
    const response = await $fetch<{ status: string; checkinCode: string }>(
      `/api/event/public/events/${slug}/registrations`,
      { method: 'POST', body: { code: code.value || undefined, answers } },
    )
    result.value = response
  } catch (error) {
    const data = (error as { data?: { detail?: string; errors?: Array<{ message: string }> } }).data
    errorMessage.value = data?.errors?.[0]?.message
      ? `${data.errors[0].message}`
      : data?.detail ?? '报名失败'
  } finally {
    submitting.value = false
  }
}

/** 状态查询。 */
async function queryStatus() {
  queryError.value = ''
  queryResult.value = null
  try {
    queryResult.value = await $fetch(
      `/api/event/public/registrations/${query.code}?email=${encodeURIComponent(query.email)}`,
    )
  } catch {
    queryError.value = '未找到报名记录（请核对签到码与邮箱）'
  }
}
</script>

<template>
  <Card class="space-y-4">
    <template v-if="event && !result">
      <div>
        <h1 class="text-lg font-semibold">{{ event.title }}</h1>
        <p v-if="event.location" class="mt-1 text-sm text-neutral-500">地点：{{ event.location }}</p>
        <p class="mt-1 text-xs text-neutral-400">
          剩余名额：{{ event.remaining === null || event.remaining > 0 ? `${event.remaining ?? '不限'}` : '已满' }}
          <span v-if="event.needReview"> · 需审核</span>
        </p>
      </div>
      <p v-if="event.descriptionMd" class="whitespace-pre-line text-sm text-neutral-600">{{ event.descriptionMd }}</p>

      <form class="space-y-3" @submit.prevent="submit">
        <template v-for="field in event.form.fields" :key="field.key">
          <label class="block">
            <span class="mb-1 block text-sm font-medium text-neutral-700">
              {{ field.label }}<span v-if="field.required" class="text-danger-500"> *</span>
            </span>
            <textarea
              v-if="field.type === 'textarea'"
              :value="answers[field.key] as string"
              @input="answers[field.key] = ($event.target as HTMLTextAreaElement).value"
              rows="3"
              class="w-full rounded-[var(--radius-field)] border border-neutral-300 p-2 text-sm"
            />
            <select
              v-else-if="field.type === 'select' || field.type === 'radio'"
              :value="answers[field.key] as string"
              @change="answers[field.key] = ($event.target as HTMLSelectElement).value"
              class="w-full rounded-[var(--radius-field)] border border-neutral-300 p-2 text-sm"
            >
              <option value="">请选择</option>
              <option v-for="option in field.options" :key="option.value" :value="option.value">{{ option.label }}</option>
            </select>
            <div v-else-if="field.type === 'multiselect'" class="space-y-1">
              <label v-for="option in field.options" :key="option.value" class="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  :value="option.value"
                  :checked="(answers[field.key] as string[]).includes(option.value)"
                  @change="(event) => {
                    const list = answers[field.key] as string[]
                    const target = event.target as HTMLInputElement
                    answers[field.key] = target.checked ? [...list, option.value] : list.filter((v) => v !== option.value)
                  }"
                />
                {{ option.label }}
              </label>
            </div>
            <label v-else-if="field.type === 'checkbox'" class="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                :checked="answers[field.key] as boolean"
                @change="answers[field.key] = ($event.target as HTMLInputElement).checked"
              />
              {{ field.label }}
            </label>
            <input
              v-else
              :value="answers[field.key] as string"
              @input="answers[field.key] = ($event.target as HTMLInputElement).value"
              :type="field.type === 'number' ? 'number' : field.type === 'date' ? 'date' : field.type === 'email' ? 'email' : 'text'"
              class="w-full rounded-[var(--radius-field)] border border-neutral-300 p-2 text-sm"
            />
          </label>
        </template>

        <div v-if="event.emailVerify" class="flex items-end gap-2">
          <Input v-model="code" label="邮箱验证码" placeholder="6 位数字" class="flex-1" />
          <Button type="button" variant="secondary" :loading="sending" @click="sendCode">发送验证码</Button>
        </div>
        <p v-if="devCode" class="text-xs text-neutral-400">开发模式验证码：{{ devCode }}</p>

        <p v-if="errorMessage" role="alert" class="text-sm text-danger-500">{{ errorMessage }}</p>
        <Button type="submit" block :loading="submitting">提交报名</Button>
      </form>
    </template>

    <template v-else-if="result">
      <h1 class="text-lg font-semibold">报名已提交</h1>
      <p class="text-sm text-neutral-600">状态：{{ result.status === 'approved' ? '已通过' : result.status === 'pending' ? '待审核' : '候补中' }}</p>
      <p class="text-sm text-neutral-600">签到码：<code class="rounded bg-neutral-100 px-2 py-0.5">{{ result.checkinCode }}</code></p>
      <p class="text-xs text-neutral-400">请保存签到码，现场出示用于签到。</p>
      <Button variant="secondary" @click="result = null">返回</Button>
    </template>

    <template v-else>
      <p class="text-sm text-danger-500">{{ errorMessage || '加载中…' }}</p>
    </template>

    <details class="border-t border-neutral-100 pt-3 text-sm">
      <summary class="cursor-pointer text-neutral-500">查询报名状态</summary>
      <div class="mt-2 space-y-2">
        <Input v-model="query.code" placeholder="签到码" />
        <Input v-model="query.email" placeholder="报名邮箱" />
        <Button size="sm" variant="secondary" @click="queryStatus">查询</Button>
        <p v-if="queryResult" class="text-sm text-neutral-600">
          {{ queryResult.statusLabel }}{{ queryResult.checkedIn ? ' · 已签到' : '' }}
        </p>
        <p v-if="queryError" class="text-sm text-danger-500">{{ queryError }}</p>
      </div>
    </details>
  </Card>
</template>
