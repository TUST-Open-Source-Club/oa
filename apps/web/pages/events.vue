<script setup lang="ts">
/** 活动管理页：活动 CRUD/开关、名单审核与签到、统计、Excel 导出。 */
import { Button, Card, Input } from '@club-oa/ui'

interface EventItem {
  id: string
  slug: string
  title: string
  status: string
  capacity: number
}
interface RegistrationItem {
  id: string
  name: string | null
  email: string
  status: string
  checkedInAt: string | null
  createdAt: string
}

const auth = useAuthStore()
const events = ref<EventItem[]>([])
const selectedId = ref('')
const registrations = ref<RegistrationItem[]>([])
const stats = ref<Record<string, number> | null>(null)
const statusFilter = ref('')
const message = ref('')
const errorMessage = ref('')

const newEvent = reactive({ slug: '', title: '', capacity: 0, needReview: false, emailVerify: true })
const createOpen = ref(false)
const origin = ref('')
const copied = ref(false)
const qrOpen = ref(false)
const qrDataUrl = ref('')
const formOpen = ref(false)
const changesOpen = ref(false)
const changes = ref<Array<{ id: string; action: string; createdAt: string }>>([])
const formSchema = ref<{ version?: number; fields: Array<Record<string, unknown>> }>({ fields: [] })
const formSaving = ref(false)

/** 报名公开链接。 */
const shareUrl = computed(() =>
  selected.value && origin.value ? `${origin.value}/e/${selected.value.slug}` : '',
)

/** 一键复制报名链接。 */
async function copyLink() {
  if (!shareUrl.value) return
  try {
    await navigator.clipboard.writeText(shareUrl.value)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch {
    message.value = '复制失败，请手动复制'
  }
}

/** 打开操作记录。 */
async function openChanges() {
  if (!selectedId.value) return
  try {
    changes.value = await api(`events/${selectedId.value}/changes`)
  } catch {
    changes.value = []
  }
  changesOpen.value = true
}

/** 撤销一次变更。 */
async function undoChange(change: { id: string }) {
  await api(`changes/${change.id}/undo`, { method: 'POST' })
  await loadEvents()
  await openChanges()
}
async function openFormDesigner() {
  if (!selectedId.value) return
  errorMessage.value = ''
  try {
    const response = await api<{ version?: number; schema?: { fields: unknown[] } } & { fields?: unknown[] }>(
      `events/${selectedId.value}/form`,
    )
    const schema = (response.schema ?? response) as { version?: number; fields: Array<Record<string, unknown>> }
    formSchema.value = { version: schema.version, fields: schema.fields ?? [] }
    formOpen.value = true
  } catch (error) {
    message.value = apiErrorMessage(error)
  }
}

/** 保存报名表单。 */
async function saveForm() {
  formSaving.value = true
  errorMessage.value = ''
  try {
    await api(`events/${selectedId.value}/form`, { method: 'PUT', body: { fields: formSchema.value.fields } })
    formOpen.value = false
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  } finally {
    formSaving.value = false
  }
}

/** 生成报名二维码。 */
async function showQr() {
  if (!shareUrl.value) return
  const { default: QRCode } = await import('qrcode')
  qrDataUrl.value = await QRCode.toDataURL(shareUrl.value, { width: 320, margin: 1 })
  qrOpen.value = true
}

const selected = computed(() => events.value.find((item) => item.id === selectedId.value) ?? null)

/** 带鉴权的 API 请求。 */
function api<T>(path: string, options: Record<string, unknown> = {}) {
  return $fetch<T>(`/api/event/${path}`, {
    ...options,
    headers: { ...auth.authHeaders(), ...(options.headers as Record<string, string> | undefined) },
  })
}

/** 加载活动列表。 */
async function loadEvents() {
  events.value = await api<EventItem[]>('events')
  if (events.value.length > 0 && !selectedId.value) selectedId.value = events.value[0].id
  if (selectedId.value) await loadDetail()
}

/** 加载名单与统计。 */
async function loadDetail() {
  const query = statusFilter.value ? `?status=${statusFilter.value}` : ''
  const [list, statistic] = await Promise.all([
    api<RegistrationItem[]>(`events/${selectedId.value}/registrations${query}`),
    api<Record<string, number>>(`events/${selectedId.value}/stats`),
  ])
  registrations.value = list
  stats.value = statistic
}

/** 创建活动。 */
async function createEvent() {
  message.value = ''
  try {
    await api('events', { method: 'POST', body: { ...newEvent } })
    newEvent.slug = ''
    newEvent.title = ''
    createOpen.value = false
    await loadEvents()
  } catch (error) {
    message.value = (error as Error).message
  }
}

/** 切换开放/关闭。 */
async function toggleStatus() {
  if (!selected.value) return
  const next = selected.value.status === 'open' ? 'closed' : 'open'
  await api(`events/${selectedId.value}`, { method: 'PATCH', body: { status: next } })
  await loadEvents()
}

/** 审核操作。 */
async function review(id: string, action: string) {
  await api(`events/${selectedId.value}/registrations/${id}/review`, {
    method: 'POST',
    body: { action },
  })
  await loadDetail()
}

/** 签到。 */
async function checkin(id: string) {
  await api(`events/${selectedId.value}/registrations/${id}/checkin`, { method: 'POST' })
  await loadDetail()
}

/** 导出 Excel（带鉴权下载）。 */
async function exportXlsx() {
  const response = await fetch(`/api/event/events/${selectedId.value}/export.xlsx`, {
    headers: auth.authHeaders(),
  })
  if (!response.ok) {
    message.value = '导出失败'
    return
  }
  const blob = await response.blob()
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = '报名表.xlsx'
  link.click()
  URL.revokeObjectURL(url)
}

/** 状态中文。 */
function statusLabel(status: string) {
  return (
    {
      approved: '已通过',
      pending: '待审核',
      rejected: '已拒绝',
      waitlist: '候补',
      cancelled: '已取消',
    }[status] ?? status
  )
}

onMounted(() => {
  origin.value = window.location.origin
  void loadEvents()
})
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between gap-2">
      <h1 class="text-xl font-semibold">活动报名</h1>
      <Button size="sm" @click="createOpen = true">
        <AppIcon name="plus" class="size-4" /> 新建活动
      </Button>
    </div>
    <p v-if="message || errorMessage" role="alert" class="text-sm text-danger-500">{{ errorMessage || message }}</p>


    <div class="grid gap-4 lg:grid-cols-[260px_1fr]">
      <Card class="space-y-2">
        <button
          v-for="item in events"
          :key="item.id"
          class="block w-full rounded px-2 py-1.5 text-left text-sm hover:bg-neutral-100"
          :class="selectedId === item.id ? 'bg-primary-50 text-primary-700' : ''"
          @click="selectedId = item.id; loadDetail()"
        >
          {{ item.title }}
          <span class="ml-1 text-xs" :class="item.status === 'open' ? 'text-success-700' : 'text-neutral-400'">
            {{ item.status === 'open' ? '开放中' : item.status === 'draft' ? '草稿' : '已关闭' }}
          </span>
        </button>
        <p v-if="events.length === 0" class="py-3 text-center text-xs text-neutral-400">暂无活动</p>
      </Card>

      <Card v-if="selected" class="space-y-3">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div>
            <h2 class="font-semibold">{{ selected.title }}</h2>
            <p class="text-xs text-neutral-400">
              报名页 /e/{{ selected.slug }} · 名额 {{ selected.capacity === 0 ? '不限' : selected.capacity }}
            </p>
          </div>
          <div class="flex items-center gap-2">
            <Button size="sm" variant="secondary" @click="openFormDesigner">表单设计</Button>
            <Button size="sm" variant="secondary" @click="openChanges">操作记录</Button>
            <Button size="sm" variant="secondary" @click="toggleStatus">
              {{ selected.status === 'open' ? '关闭报名' : '开放报名' }}
            </Button>
            <Button size="sm" variant="secondary" @click="exportXlsx">导出 Excel</Button>
          </div>
        </div>

        <div
          class="flex flex-wrap items-center gap-2 rounded-[var(--radius-field)] bg-neutral-50 px-3 py-2 dark:bg-neutral-800/50"
        >
          <span class="text-xs text-neutral-400">报名链接</span>
          <input
            readonly
            :value="shareUrl"
            class="min-w-0 flex-1 bg-transparent text-xs text-neutral-600 outline-none dark:text-neutral-300"
            @focus="($event.target as HTMLInputElement).select()"
          />
          <Button size="sm" variant="secondary" @click="copyLink">
            {{ copied ? '已复制' : '复制' }}
          </Button>
          <Button size="sm" variant="secondary" @click="showQr">二维码</Button>
          <a
            :href="shareUrl"
            target="_blank"
            class="rounded-[var(--radius-field)] px-2 py-1.5 text-xs text-primary-600 transition hover:bg-primary-50 dark:hover:bg-primary-900/30"
          >
            预览
          </a>
        </div>

        <div v-if="stats" class="flex flex-wrap gap-4 rounded bg-neutral-50 px-3 py-2 text-sm dark:bg-neutral-800/50">
          <span>总数 {{ stats.total }}</span>
          <span class="text-success-700">已通过 {{ stats.approved }}</span>
          <span class="text-warning-700">待审核 {{ stats.pending }}</span>
          <span>候补 {{ stats.waitlist }}</span>
          <span>已签到 {{ stats.checkedIn }}</span>
        </div>

        <div class="flex items-center gap-2 text-sm">
          <select v-model="statusFilter" class="rounded-[var(--radius-field)] border border-neutral-300 px-2 py-1.5" @change="loadDetail">
            <option value="">全部状态</option>
            <option value="approved">已通过</option>
            <option value="pending">待审核</option>
            <option value="waitlist">候补</option>
            <option value="rejected">已拒绝</option>
            <option value="cancelled">已取消</option>
          </select>
        </div>

        <div class="divide-y divide-neutral-100 text-sm">
          <div v-for="row in registrations" :key="row.id" class="flex flex-wrap items-center justify-between gap-2 py-2">
            <div class="min-w-0">
              <span class="font-medium">{{ row.name || '未填写' }}</span>
              <span class="ml-2 text-neutral-500">{{ row.email }}</span>
              <span class="ml-2 text-xs" :class="row.status === 'approved' ? 'text-success-700' : 'text-neutral-400'">
                {{ statusLabel(row.status) }}{{ row.checkedInAt ? ' · 已签到' : '' }}
              </span>
            </div>
            <div class="flex shrink-0 gap-1">
              <Button v-if="row.status === 'pending' || row.status === 'waitlist'" variant="ghost" size="sm" @click="review(row.id, 'approve')">通过</Button>
              <Button v-if="row.status === 'pending'" variant="ghost" size="sm" @click="review(row.id, 'reject')">拒绝</Button>
              <Button v-if="row.status === 'approved'" variant="ghost" size="sm" @click="review(row.id, 'cancel')">取消</Button>
              <Button v-if="row.status === 'approved' && !row.checkedInAt" variant="ghost" size="sm" @click="checkin(row.id)">签到</Button>
            </div>
          </div>
          <p v-if="registrations.length === 0" class="py-3 text-center text-xs text-neutral-400">暂无报名</p>
        </div>
      </Card>

      <Card v-else class="text-sm text-neutral-400">选择或创建一个活动</Card>
    </div>

    <AppModal :open="createOpen" title="新建活动" @close="createOpen = false">
      <div class="space-y-3">
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">标题</span>
          <Input v-model="newEvent.title" placeholder="活动标题" />
        </label>
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">slug（报名链接 /e/&lt;slug&gt;）</span>
          <Input v-model="newEvent.slug" placeholder="小写字母/数字/-" />
        </label>
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">名额</span>
          <input
            v-model.number="newEvent.capacity"
            type="number"
            min="0"
            class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700"
            placeholder="0 = 不限"
          />
        </label>
        <div class="flex gap-4 text-sm">
          <label class="flex items-center gap-1.5"><input v-model="newEvent.needReview" type="checkbox" /> 需审核</label>
          <label class="flex items-center gap-1.5"><input v-model="newEvent.emailVerify" type="checkbox" /> 邮箱验证</label>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="!newEvent.title.trim() || !newEvent.slug.trim()" @click="createEvent">创建</Button>
      </template>
    </AppModal>

    <AppModal :open="formOpen" title="报名表单设计" width="max-w-2xl" @close="formOpen = false">
      <FormBuilder v-model="formSchema" />
      <p class="mt-3 text-xs text-neutral-400">
        提示：key 为 name/email/phone 的字段会自动同步到报名记录的联系信息列。
      </p>
      <template #footer>
        <Button variant="secondary" @click="formOpen = false">取消</Button>
        <Button :loading="formSaving" @click="saveForm">保存表单</Button>
      </template>
    </AppModal>

    <AppModal :open="changesOpen" title="操作记录" @close="changesOpen = false">
      <div v-if="changes.length" class="space-y-1.5">
        <div
          v-for="change in changes"
          :key="change.id"
          class="flex items-center justify-between rounded-[var(--radius-field)] border border-neutral-200 px-2.5 py-1.5 text-sm dark:border-neutral-700"
        >
          <span class="text-neutral-500">
            {{ change.action === 'update' ? '编辑' : change.action === 'form' ? '表单变更' : change.action === 'undo' ? '撤销' : change.action }}
            · {{ new Date(change.createdAt).toLocaleString('zh-CN') }}
          </span>
          <button class="text-primary-600 hover:underline" @click="undoChange(change)">撤销</button>
        </div>
      </div>
      <p v-else class="text-sm text-neutral-400">暂无操作记录</p>
    </AppModal>

    <AppModal :open="qrOpen" title="报名二维码" @close="qrOpen = false">
      <div class="flex flex-col items-center gap-3">
        <img
          v-if="qrDataUrl"
          :src="qrDataUrl"
          alt="报名二维码"
          class="size-64 rounded-lg border border-neutral-200 bg-white p-2 dark:border-neutral-700"
        />
        <p class="break-all text-center text-xs text-neutral-400">{{ shareUrl }}</p>
        <p class="text-xs text-neutral-400">扫码即可打开报名页，可保存图片后转发</p>
      </div>
    </AppModal>
  </div>
</template>
