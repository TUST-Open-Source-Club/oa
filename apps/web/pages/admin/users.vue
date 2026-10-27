<script setup lang="ts">
/** 用户管理（管理员）：创建人类账号/Bot、Bot 权限矩阵、禁用与重置密码。 */
import { Button, Card, Input } from '@club-oa/ui'

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

/** 加载用户列表。 */
async function loadUsers() {
  loading.value = true
  message.value = ''
  try {
    const response = await api<{ items: UserRow[]; total: number }>('users', {
      query: { page: page.value, pageSize: 20, q: keyword.value || undefined },
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

onMounted(loadUsers)
</script>

<template>
  <div class="space-y-4">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h1 class="text-xl font-semibold">用户管理</h1>
        <p class="mt-0.5 text-sm text-neutral-400">共 {{ total }} 个账号；Bot 可精确配置各模块读写权限</p>
      </div>
      <div class="flex items-center gap-2">
        <Input v-model="keyword" placeholder="搜索用户名/邮箱/昵称" class="max-w-56" @keyup.enter="page = 1; loadUsers()" />
        <Button variant="secondary" size="sm" @click="page = 1; loadUsers()">搜索</Button>
        <Button size="sm" @click="createOpen = true">
          <AppIcon name="plus" class="size-4" /> 新建账号
        </Button>
      </div>
    </div>

    <p v-if="message" class="text-sm text-neutral-500">{{ message }}</p>

    <Card class="divide-y divide-neutral-100 dark:divide-neutral-800">
      <div v-for="user in users" :key="user.id" class="flex flex-wrap items-center gap-3 py-2.5 text-sm">
        <UserAvatar :name="user.nickname" :seed="user.id" size="sm" />
        <div class="min-w-0 flex-1">
          <p class="truncate font-medium">
            {{ user.nickname }}
            <span v-if="user.accountType === 'bot'" class="ml-1 rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40">Bot</span>
          </p>
          <p class="truncate text-xs text-neutral-400">
            @{{ user.username }} · {{ user.email }} · {{ user.status === 'active' ? '正常' : user.status === 'disabled' ? '已禁用' : '待激活' }}
          </p>
        </div>
        <div class="flex shrink-0 gap-1">
          <Button v-if="user.accountType === 'bot'" variant="ghost" size="sm" @click="openPermissions(user)">权限</Button>
          <Button variant="ghost" size="sm" @click="toggleStatus(user)">
            {{ user.status === 'active' ? '禁用' : '启用' }}
          </Button>
          <Button variant="ghost" size="sm" @click="resetPassword(user)">重置密码</Button>
        </div>
      </div>
      <p v-if="!users.length && !loading" class="py-8 text-center text-sm text-neutral-400">暂无用户</p>
    </Card>

    <!-- 新建账号 -->
    <AppModal :open="createOpen" title="新建账号" @close="createOpen = false">
      <div class="space-y-3">
        <div class="flex gap-2">
          <button
            v-for="option in [{ value: 'human', label: '成员账号' }, { value: 'bot', label: 'Bot 账号' }]"
            :key="option.value"
            class="flex-1 rounded-[var(--radius-field)] border px-3 py-2 text-sm transition"
            :class="createForm.accountType === option.value ? 'border-primary-500 bg-primary-50 text-primary-700 dark:bg-primary-900/40' : 'border-neutral-200 text-neutral-500 dark:border-neutral-700'"
            @click="createForm.accountType = option.value"
          >
            {{ option.label }}
          </button>
        </div>
        <Input v-model="createForm.email" placeholder="邮箱" />
        <Input v-model="createForm.username" placeholder="登录名" />
        <Input v-model="createForm.nickname" placeholder="昵称" />
        <template v-if="createForm.accountType === 'bot'">
          <Input v-model="createForm.password" type="password" placeholder="Bot 密码（≥8 位）" />
          <div class="space-y-1.5 rounded-[var(--radius-field)] border border-neutral-200 p-3 text-sm dark:border-neutral-700">
            <p class="text-xs text-neutral-500">模块权限</p>
            <div v-for="module in modules" :key="module" class="flex items-center justify-between">
              <span>{{ moduleLabels[module] }}</span>
              <span class="flex gap-3 text-xs">
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
        <Input v-else v-model="createForm.roles" placeholder="角色（逗号分隔，默认 member）" />
      </div>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="!createForm.email.trim() || !createForm.username.trim()" @click="createUser">创建</Button>
      </template>
    </AppModal>

    <!-- Bot 权限矩阵 -->
    <AppModal :open="permOpen" title="Bot 权限矩阵" @close="permOpen = false">
      <p class="mb-3 text-sm text-neutral-500">{{ permTarget?.nickname }}</p>
      <div class="space-y-2 text-sm">
        <div v-for="module in modules" :key="module" class="flex items-center justify-between rounded-[var(--radius-field)] border border-neutral-200 px-3 py-2 dark:border-neutral-700">
          <span>{{ moduleLabels[module] }}</span>
          <span class="flex gap-3 text-xs">
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
