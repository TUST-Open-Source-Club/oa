<script setup lang="ts">
/** 用户管理（管理员）：创建人类账号/Bot、Bot 权限矩阵、禁用与重置密码。 */
import { Button, Input } from '@club-oa/ui'

/** 用户行。 */
interface UserRow {
  id: string
  username: string
  email: string
  nickname: string
  department: string | null
  status: string
  roles: string[]
  accountType: string
  botPermissions: Record<string, { read: boolean; write: boolean }>
}

const auth = useAuthStore()
const modules = ['im', 'task', 'doc', 'event', 'drive', 'notify']
const moduleLabels: Record<string, string> = {
  im: '消息', task: '任务', doc: '文档', event: '活动', drive: '网盘', notify: '通知',
}
const PAGE_SIZE = 20

const users = ref<UserRow[]>([])
const total = ref(0)
const page = ref(1)
const keyword = ref('')
const message = ref('')
const loading = ref(false)

const createOpen = ref(false)
const createForm = reactive({
  email: '',
  username: '',
  nickname: '',
  accountType: 'human',
  password: '',
  roles: 'member',
  permissions: {} as Record<string, { read: boolean; write: boolean }>,
})
const permOpen = ref(false)
const permTarget = ref<UserRow | null>(null)
const permDraft = ref<Record<string, { read: boolean; write: boolean }>>({})

/** 带鉴权请求。 */
function api<T>(path: string, options: Record<string, unknown> = {}) {
  return $fetch<T>(`/api/auth/admin/${path}`, {
    ...options,
    headers: { ...auth.authHeaders(), ...((options.headers as Record<string, string> | undefined) ?? {}) },
  })
}

/** 状态中文名。 */
function statusLabel(status: string): string {
  if (status === 'active') return '正常'
  if (status === 'disabled') return '已禁用'
  return '待激活'
}

/** 状态徽标配色。 */
function statusClass(status: string): string {
  if (status === 'active') return 'bg-success-50 text-success-700'
  if (status === 'disabled') return 'bg-neutral-100 text-neutral-500 dark:bg-neutral-800'
  return 'bg-warning-50 text-warning-700'
}

/** 加载用户列表。 */
async function loadUsers() {
  loading.value = true
  message.value = ''
  try {
    const response = await api<{ items: UserRow[]; total: number }>('users', {
      query: { page: page.value, pageSize: PAGE_SIZE, q: keyword.value || undefined },
    })
    users.value = response.items
    total.value = response.total
  } catch (error) {
    message.value = apiErrorMessage(error)
  } finally {
    loading.value = false
  }
}

/** 创建账号（人类或 Bot）。 */
async function createUser() {
  message.value = ''
  try {
    const body: Record<string, unknown> = {
      email: createForm.email,
      username: createForm.username,
      nickname: createForm.nickname,
      accountType: createForm.accountType,
    }
    if (createForm.accountType === 'bot') {
      body.password = createForm.password
      body.botPermissions = createForm.permissions
    } else {
      body.roles = createForm.roles.split(',').map((role) => role.trim()).filter(Boolean)
    }
    const created = await api<{ user: UserRow; devActivationUrl?: string }>('users', { method: 'POST', body })
    createOpen.value = false
    Object.assign(createForm, { email: '', username: '', nickname: '', password: '', roles: 'member', permissions: {} })
    message.value = created.devActivationUrl ? `已创建，激活链接：${created.devActivationUrl}` : '已创建'
    await loadUsers()
  } catch (error) {
    message.value = apiErrorMessage(error)
  }
}

/** 打开 Bot 权限矩阵编辑。 */
function openPermissions(user: UserRow) {
  permTarget.value = user
  permDraft.value = JSON.parse(JSON.stringify(user.botPermissions ?? {}))
  permOpen.value = true
}

/** 保存 Bot 权限矩阵。 */
async function savePermissions() {
  if (!permTarget.value) return
  await api(`users/${permTarget.value.id}`, { method: 'PATCH', body: { botPermissions: permDraft.value } })
  permOpen.value = false
  await loadUsers()
}

/** 切换启用/禁用。 */
async function toggleStatus(user: UserRow) {
  await api(`users/${user.id}`, {
    method: 'PATCH',
    body: { status: user.status === 'active' ? 'disabled' : 'active' },
  })
  await loadUsers()
}

/** 重置密码（返回开发模式临时密码/链接）。 */
async function resetPassword(user: UserRow) {
  if (!window.confirm(`重置「${user.nickname}」的密码？`)) return
  const result = await api<Record<string, unknown>>(`users/${user.id}/reset-password`, { method: 'POST' })
  message.value = `已重置${result.devResetUrl ? `，链接：${result.devResetUrl}` : ''}`
}

/** 搜索：回到第一页。 */
function onSearch() {
  page.value = 1
  void loadUsers()
}

onMounted(loadUsers)
</script>

<template>
  <div class="mx-auto w-full max-w-[960px]">
    <header class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex items-baseline gap-2">
        <p class="text-[12px] text-neutral-500">
          共 {{ total }} 个账号 · Bot 可精确配置模块读写
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Input v-model="keyword" placeholder="搜索用户名/邮箱/昵称" class="w-56" autocomplete="off" @keyup.enter="onSearch" />
        <Button variant="secondary" size="sm" @click="onSearch">搜索</Button>
        <Button size="sm" @click="createOpen = true">
          <AppIcon name="plus" class="size-3.5" />
          新建账号
        </Button>
      </div>
    </header>

    <p
      v-if="message"
      class="mt-3 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] px-3 py-2 text-[12px] break-all text-neutral-600 dark:text-neutral-300"
    >
      {{ message }}
    </p>

    <div
      class="mt-4 overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <article
        v-for="(user, index) in users"
        :key="user.id"
        class="flex flex-wrap items-center gap-3 px-4 py-3 transition-colors hover:bg-[var(--surface-hover)]"
        :class="index > 0 ? 'border-t border-[var(--border)]' : ''"
      >
        <UserAvatar :name="user.nickname" :seed="user.id" size="sm" />
        <div class="min-w-0 flex-1">
          <p class="flex items-center gap-1.5 truncate text-[13px] font-medium text-[var(--text-strong)]">
            {{ user.nickname }}
            <span
              v-if="user.accountType === 'bot'"
              class="rounded border border-primary-200 bg-primary-50 px-1 text-[10px] font-medium text-primary-700 dark:border-primary-800 dark:bg-primary-900/30 dark:text-primary-200"
            >
              Bot
            </span>
          </p>
          <p class="mt-0.5 truncate text-[12px] text-neutral-400">
            @{{ user.username }} · {{ user.email }}
          </p>
        </div>
        <span
          class="shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium"
          :class="statusClass(user.status)"
        >
          {{ statusLabel(user.status) }}
        </span>
        <div class="flex shrink-0 items-center gap-0.5">
          <Button v-if="user.accountType === 'bot'" variant="ghost" size="sm" @click="openPermissions(user)">
            权限
          </Button>
          <Button variant="ghost" size="sm" @click="toggleStatus(user)">
            {{ user.status === 'active' ? '禁用' : '启用' }}
          </Button>
          <Button variant="ghost" size="sm" @click="resetPassword(user)">重置密码</Button>
        </div>
      </article>
      <p v-if="!users.length && !loading" class="px-4 py-12 text-center text-[12px] text-neutral-400">
        暂无用户
      </p>
    </div>

    <div v-if="total > PAGE_SIZE" class="mt-3 flex items-center justify-between text-[12px] text-neutral-500">
      <span>第 {{ page }} 页 · 共 {{ Math.ceil(total / PAGE_SIZE) }} 页</span>
      <div class="flex gap-1.5">
        <Button variant="secondary" size="sm" :disabled="page <= 1" @click="page -= 1; loadUsers()">
          上一页
        </Button>
        <Button
          variant="secondary"
          size="sm"
          :disabled="page * PAGE_SIZE >= total"
          @click="page += 1; loadUsers()"
        >
          下一页
        </Button>
      </div>
    </div>

    <!-- 新建账号 -->
    <AppModal :open="createOpen" title="新建账号" @close="createOpen = false">
      <div class="space-y-3">
        <div class="grid grid-cols-2 gap-1 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] p-1">
          <button
            v-for="option in [{ value: 'human', label: '成员账号' }, { value: 'bot', label: 'Bot 账号' }]"
            :key="option.value"
            class="rounded-[calc(var(--radius-field)-2px)] px-3 py-1.5 text-[12px] font-medium transition"
            :class="
              createForm.accountType === option.value
                ? 'bg-[var(--surface)] text-[var(--text-strong)] shadow-[var(--shadow-card)]'
                : 'text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-200'
            "
            @click="createForm.accountType = option.value"
          >
            {{ option.label }}
          </button>
        </div>
        <Input v-model="createForm.email" label="邮箱" placeholder="alice@club.example.com" />
        <Input v-model="createForm.username" label="登录名" placeholder="alice" />
        <Input v-model="createForm.nickname" label="昵称" placeholder="张三" />
        <template v-if="createForm.accountType === 'bot'">
          <Input v-model="createForm.password" label="Bot 密码" type="password" placeholder="至少 8 位" />
          <div class="space-y-2 rounded-[var(--radius-field)] border border-[var(--border)] p-3">
            <p class="text-[11px] font-medium tracking-[0.08em] text-neutral-400 uppercase">模块权限</p>
            <div
              v-for="module in modules"
              :key="module"
              class="flex items-center justify-between text-[12px]"
            >
              <span class="text-neutral-600 dark:text-neutral-300">{{ moduleLabels[module] }}</span>
              <span class="flex gap-3 text-[11px] text-neutral-500">
                <label class="flex items-center gap-1">
                  <input
                    type="checkbox"
                    :checked="createForm.permissions[module]?.read ?? false"
                    @change="createForm.permissions[module] = { read: ($event.target as HTMLInputElement).checked, write: createForm.permissions[module]?.write ?? false }"
                  /> 读
                </label>
                <label class="flex items-center gap-1">
                  <input
                    type="checkbox"
                    :checked="createForm.permissions[module]?.write ?? false"
                    @change="createForm.permissions[module] = { read: createForm.permissions[module]?.read ?? false, write: ($event.target as HTMLInputElement).checked }"
                  /> 写
                </label>
              </span>
            </div>
          </div>
        </template>
        <Input v-else v-model="createForm.roles" label="角色" placeholder="逗号分隔，默认 member" />
      </div>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="!createForm.email.trim() || !createForm.username.trim()" @click="createUser">创建</Button>
      </template>
    </AppModal>

    <!-- Bot 权限矩阵 -->
    <AppModal :open="permOpen" title="Bot 权限矩阵" @close="permOpen = false">
      <p class="mb-3 text-[12px] text-neutral-500">{{ permTarget?.nickname }}</p>
      <div class="space-y-1.5">
        <div
          v-for="module in modules"
          :key="module"
          class="flex items-center justify-between rounded-[var(--radius-field)] border border-[var(--border)] px-3 py-1.5 text-[12px]"
        >
          <span class="text-neutral-600 dark:text-neutral-300">{{ moduleLabels[module] }}</span>
          <span class="flex gap-3 text-[11px] text-neutral-500">
            <label class="flex items-center gap-1">
              <input
                type="checkbox"
                :checked="permDraft[module]?.read ?? false"
                @change="permDraft[module] = { read: ($event.target as HTMLInputElement).checked, write: permDraft[module]?.write ?? false }"
              /> 读
            </label>
            <label class="flex items-center gap-1">
              <input
                type="checkbox"
                :checked="permDraft[module]?.write ?? false"
                @change="permDraft[module] = { read: permDraft[module]?.read ?? false, write: ($event.target as HTMLInputElement).checked }"
              /> 写
            </label>
          </span>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="permOpen = false">取消</Button>
        <Button @click="savePermissions">保存</Button>
      </template>
    </AppModal>
  </div>
</template>
