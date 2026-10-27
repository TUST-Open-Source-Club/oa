<script setup lang="ts">
/** 消息页：会话列表 + 消息流 + 发送 + 发起会话（4s 轮询增量）。 */
import { Button } from '@club-oa/ui'

/** 用户条目。 */
interface UserItem {
  id: string
  username: string
  nickname: string
  department: string | null
}
/** 消息条目。 */
interface MessageItem {
  id: string
  seq: number
  senderId: string | null
  type: string
  content: { text?: string }
  createdAt: string
}
/** 会话条目。 */
interface ConversationItem {
  id: string
  type: string
  name: string | null
  notice?: string | null
  onlyAdminsSpeak?: boolean
  unread: number
  memberIds: string[]
  lastMessage: MessageItem | null
  updatedAt: string
}
/** 群成员行。 */
interface MemberRow {
  userId: string
  role: string
  mutedUntil: string | null
  canSpeak: boolean
}

const auth = useAuthStore()
const conversations = ref<ConversationItem[]>([])
const activeId = ref('')
const messages = ref<MessageItem[]>([])
const users = ref<Record<string, UserItem>>({})
const text = ref('')
const error = ref('')
const sending = ref(false)
const listEl = ref<HTMLElement | null>(null)
const createOpen = ref(false)
const pickedIds = ref<string[]>([])
const groupName = ref('')
const msgMenu = reactive<{ open: boolean; x: number; y: number; message: MessageItem | null }>({
  open: false,
  x: 0,
  y: 0,
  message: null,
})
const convMenu = reactive<{ open: boolean; x: number; y: number; conversation: ConversationItem | null }>({
  open: false,
  x: 0,
  y: 0,
  conversation: null,
})
const membersOpen = ref(false)
const members = ref<MemberRow[]>([])
const groupSettings = reactive({ onlyAdminsSpeak: false, notice: '' })
const myRole = computed(() => members.value.find((row) => row.userId === auth.user?.id)?.role ?? 'member')
const isGroupAdmin = computed(() => ['owner', 'admin'].includes(myRole.value))
const isOwner = computed(() => myRole.value === 'owner')

const active = computed(() => conversations.value.find((item) => item.id === activeId.value) ?? null)

/** 带鉴权请求。 */
function api<T>(path: string, options: Record<string, unknown> = {}) {
  return $fetch<T>(`/api/im/${path}`, {
    ...options,
    headers: {
      ...auth.authHeaders(),
      ...((options.headers as Record<string, string> | undefined) ?? {}),
    },
  })
}

/** 会话显示名。 */
function titleOf(conversation: ConversationItem) {
  if (conversation.name) return conversation.name
  const other = conversation.memberIds.find((id) => id !== auth.user?.id)
  return other ? (users.value[other]?.nickname ?? '私聊') : '私聊'
}

/** 批量解析用户昵称。 */
async function resolveUsers(ids: string[]) {
  const missing = [...new Set(ids)].filter((id) => id && !users.value[id])
  if (missing.length === 0) return
  try {
    const items = await $fetch<UserItem[]>('/api/auth/users', {
      params: { ids: missing.join(',') },
      headers: auth.authHeaders(),
    })
    for (const user of items) users.value[user.id] = user
  } catch {
    // 昵称解析失败不影响消息展示
  }
}

/** 加载会话列表。 */
async function loadConversations() {
  conversations.value = await api<ConversationItem[]>('conversations')
  await resolveUsers(
    conversations.value.flatMap((item) => [
      ...item.memberIds,
      ...(item.lastMessage?.senderId ? [item.lastMessage.senderId] : []),
    ]),
  )
  if (!activeId.value && conversations.value.length > 0) {
    await openConversation(conversations.value[0].id)
  }
}

/** 加载当前会话消息。 */
async function loadMessages(scroll = true) {
  if (!activeId.value) return
  messages.value = await api<MessageItem[]>(`conversations/${activeId.value}/messages?limit=100`)
  await resolveUsers(messages.value.map((item) => item.senderId ?? ''))
  if (scroll) {
    await nextTick()
    listEl.value?.scrollTo({ top: listEl.value.scrollHeight })
  }
}

/** 打开会话并标记已读（上报已读到的最新 seq）。 */
async function openConversation(id: string) {
  activeId.value = id
  await loadMessages()
  const conversation = conversations.value.find((item) => item.id === id)
  if (!conversation) return
  if (conversation.type === 'group') void loadMembers()
  const seq = Math.max(
    conversation.lastMessage?.seq ?? 0,
    messages.value[messages.value.length - 1]?.seq ?? 0,
  )
  if (conversation.unread > 0 && seq > 0) {
    try {
      await api(`conversations/${id}/read`, { method: 'POST', body: { seq } })
      conversation.unread = 0
    } catch (err) {
      error.value = apiErrorMessage(err)
    }
  }
}

/** 发送文本消息。 */
async function send() {
  const content = text.value.trim()
  if (!content || !activeId.value || sending.value) return
  sending.value = true
  error.value = ''
  try {
    await api(`conversations/${activeId.value}/messages`, {
      method: 'POST',
      body: { type: 'text', content: { text: content } },
    })
    text.value = ''
    await loadMessages()
    await loadConversations()
  } catch (err) {
    error.value = (err as Error).message
  } finally {
    sending.value = false
  }
}

/** 发起会话（单选=私聊，多选=群聊）。 */
async function createConversation() {
  if (pickedIds.value.length === 0) return
  const isGroup = pickedIds.value.length > 1
  if (isGroup && !groupName.value.trim()) {
    error.value = '群聊需要填写名称'
    return
  }
  error.value = ''
  try {
    const created = await api<ConversationItem>('conversations', {
      method: 'POST',
      body: {
        type: isGroup ? 'group' : 'direct',
        memberIds: pickedIds.value,
        name: isGroup ? groupName.value.trim() : undefined,
      },
    })
    createOpen.value = false
    pickedIds.value = []
    groupName.value = ''
    await loadConversations()
    await openConversation(created.id)
  } catch (err) {
    error.value = (err as Error).message
  }
}

/** 加载群成员（群聊时）。 */
async function loadMembers() {
  if (active.value?.type !== 'group') {
    members.value = []
    return
  }
  try {
    members.value = await api<MemberRow[]>(`conversations/${activeId.value}/members`)
  } catch {
    members.value = []
  }
}

/** 打开群设置面板。 */
async function openGroupSettings() {
  if (!active.value || active.value.type !== 'group') return
  await loadMembers()
  groupSettings.onlyAdminsSpeak = active.value.onlyAdminsSpeak ?? false
  groupSettings.notice = active.value.notice ?? ''
  membersOpen.value = true
}

/** 保存群设置。 */
async function saveGroupSettings() {
  await api(`conversations/${activeId.value}/settings`, {
    method: 'POST',
    body: { onlyAdminsSpeak: groupSettings.onlyAdminsSpeak, notice: groupSettings.notice || null },
  })
  membersOpen.value = false
  await loadConversations()
}

/** 成员是否处于禁言中。 */
function isMuted(member: MemberRow) {
  return Boolean(member.mutedUntil && new Date(member.mutedUntil) > new Date())
}

/** 禁言/解除。 */
async function muteMember(member: MemberRow, minutes: number | null) {
  await api(`conversations/${activeId.value}/members/${member.userId}/mute`, {
    method: 'POST',
    body: { mutedUntil: minutes ? new Date(Date.now() + minutes * 60_000).toISOString() : null },
  })
  await loadMembers()
}

/** 切换发言白名单。 */
async function toggleSpeak(member: MemberRow) {
  await api(`conversations/${activeId.value}/members/${member.userId}/speak`, {
    method: 'POST',
    body: { canSpeak: !member.canSpeak },
  })
  await loadMembers()
}

/** 设置/取消管理员。 */
async function toggleAdmin(member: MemberRow) {
  await api(`conversations/${activeId.value}/members/${member.userId}/role`, {
    method: 'POST',
    body: { role: member.role === 'admin' ? 'member' : 'admin' },
  })
  await loadMembers()
}

/** 转让群主。 */
async function transferOwner(member: MemberRow) {
  const name = users.value[member.userId]?.nickname ?? '该成员'
  if (!window.confirm(`确定把群主转让给「${name}」？你将变为管理员。`)) return
  await api(`conversations/${activeId.value}/transfer`, { method: 'POST', body: { userId: member.userId } })
  await loadMembers()
  await loadConversations()
}

/** 解散群聊。 */
async function dissolveGroup() {
  if (!window.confirm('确定解散该群聊？所有消息会被删除且不可恢复。')) return
  await api(`conversations/${activeId.value}`, { method: 'DELETE' })
  membersOpen.value = false
  activeId.value = ''
  messages.value = []
  await loadConversations()
}

/** 打开消息右键菜单。 */
function openMsgMenu(event: MouseEvent, message: MessageItem) {
  msgMenu.open = true
  msgMenu.x = event.clientX
  msgMenu.y = event.clientY
  msgMenu.message = message
  convMenu.open = false
}

/** 打开会话右键菜单。 */
function openConvMenu(event: MouseEvent, conversation: ConversationItem) {
  convMenu.open = true
  convMenu.x = event.clientX
  convMenu.y = event.clientY
  convMenu.conversation = conversation
  msgMenu.open = false
}

/** 关闭所有右键菜单。 */
function closeMenus() {
  msgMenu.open = false
  convMenu.open = false
}

/** 复制消息文本。 */
async function copyMessage(message: MessageItem) {
  await navigator.clipboard.writeText(message.content?.text ?? '')
  closeMenus()
}

/** 撤回消息（服务端校验仅本人或群管理员可撤回）。 */
async function recallMessage(message: MessageItem) {
  try {
    await api(`conversations/${activeId.value}/messages/${message.id}/recall`, { method: 'POST' })
    await loadMessages()
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    closeMenus()
  }
}

/** 会话标记已读。 */
async function markConversationRead(conversation: ConversationItem) {
  const seq = conversation.lastMessage?.seq ?? 0
  if (seq > 0) {
    await api(`conversations/${conversation.id}/read`, { method: 'POST', body: { seq } }).catch(() => undefined)
    conversation.unread = 0
  }
  closeMenus()
}

/** 时间显示（今天显示时分，否则日期）。 */
function timeOf(value: string) {
  const date = new Date(value)
  const today = new Date()
  return date.toDateString() === today.toDateString()
    ? date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
    : date.toLocaleDateString('zh-CN')
}

/** 预览文本。 */
function previewOf(conversation: ConversationItem) {
  const last = conversation.lastMessage
  if (!last) return '暂无消息'
  if (last.type === 'text') return last.content?.text ?? ''
  return `[${last.type}]`
}

let pollTimer: ReturnType<typeof setInterval> | undefined
onMounted(async () => {
  window.addEventListener('click', closeMenus)
  await loadConversations()
  pollTimer = setInterval(() => {
    void loadConversations().catch(() => undefined)
    void loadMessages(false).catch(() => undefined)
  }, 4000)
})
onUnmounted(() => {
  clearInterval(pollTimer)
  window.removeEventListener('click', closeMenus)
})
</script>

<template>
  <div class="flex h-[calc(100vh-7.5rem)] overflow-hidden rounded-[var(--radius-card)] border border-neutral-200 bg-white shadow-[var(--shadow-card)] dark:border-neutral-800 dark:bg-neutral-900">
    <!-- 会话列表 -->
    <aside class="flex w-64 shrink-0 flex-col border-r border-neutral-200 md:w-72 dark:border-neutral-800">
      <div class="flex items-center justify-between px-4 py-3">
        <h1 class="text-sm font-semibold">消息</h1>
        <Button size="sm" variant="secondary" @click="createOpen = true">
          <AppIcon name="plus" class="size-4" />
        </Button>
      </div>
      <div class="flex-1 overflow-y-auto px-2 pb-2">
        <button
          v-for="conversation in conversations"
          :key="conversation.id"
          class="mb-1 flex w-full items-center gap-2.5 rounded-[var(--radius-field)] px-2.5 py-2 text-left transition"
          :class="
            conversation.id === activeId
              ? 'bg-primary-50 dark:bg-primary-900/40'
              : 'hover:bg-neutral-100 dark:hover:bg-neutral-800'
          "
          @click="openConversation(conversation.id)"
          @contextmenu.prevent="openConvMenu($event, conversation)"
        >
          <UserAvatar
            :name="titleOf(conversation)"
            :seed="conversation.id"
            size="lg"
          />
          <span class="min-w-0 flex-1">
            <span class="flex items-center justify-between gap-2">
              <span class="truncate text-sm font-medium">{{ titleOf(conversation) }}</span>
              <span class="shrink-0 text-[10px] text-neutral-400">
                {{ conversation.lastMessage ? timeOf(conversation.lastMessage.createdAt) : '' }}
              </span>
            </span>
            <span class="mt-0.5 flex items-center justify-between gap-2">
              <span class="truncate text-xs text-neutral-400">{{ previewOf(conversation) }}</span>
              <span
                v-if="conversation.unread > 0"
                class="shrink-0 rounded-full bg-danger-500 px-1.5 text-[10px] leading-4 text-white"
              >
                {{ conversation.unread > 99 ? '99+' : conversation.unread }}
              </span>
            </span>
          </span>
        </button>
        <p v-if="conversations.length === 0" class="px-3 py-6 text-center text-xs text-neutral-400">
          还没有会话，点击右上角发起
        </p>
      </div>
    </aside>

    <!-- 消息区 -->
    <section class="flex min-w-0 flex-1 flex-col">
      <template v-if="active">
        <header class="flex items-center gap-3 border-b border-neutral-200 px-4 py-3 dark:border-neutral-800">
          <UserAvatar :name="titleOf(active)" :seed="active.id" />
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-sm font-semibold">{{ titleOf(active) }}</h2>
            <p class="text-xs text-neutral-400">
              {{ active.type === 'group' ? `${active.memberIds.length} 位成员` : '私聊' }}
            </p>
          </div>
          <button
            v-if="active.type === 'group'"
            class="rounded-[var(--radius-field)] p-1.5 text-neutral-400 transition hover:bg-neutral-100 hover:text-neutral-600 dark:hover:bg-neutral-800"
            title="群设置"
            @click="openGroupSettings"
          >
            <AppIcon name="settings" class="size-4.5" />
          </button>
        </header>
        <div ref="listEl" class="flex-1 space-y-3 overflow-y-auto px-4 py-4">
          <div
            v-for="message in messages"
            :key="message.id"
            class="flex items-start gap-2"
            :class="message.senderId === auth.user?.id ? 'flex-row-reverse' : ''"
          >
            <UserAvatar
              :name="users[message.senderId ?? '']?.nickname ?? '系统'"
              :seed="message.senderId ?? 'system'"
              size="sm"
            />
            <div
              class="max-w-[70%] rounded-2xl px-3.5 py-2 text-sm"
              @contextmenu.prevent="openMsgMenu($event, message)"
              :class="
                message.senderId === auth.user?.id
                  ? 'rounded-br-sm bg-primary-600 text-white'
                  : 'rounded-bl-sm bg-neutral-100 text-neutral-800 dark:bg-neutral-800 dark:text-neutral-100'
              "
            >
              <p class="mb-0.5 text-[10px] opacity-70">
                {{ message.senderId === auth.user?.id ? '我' : (users[message.senderId ?? '']?.nickname ?? '系统') }}
                · {{ timeOf(message.createdAt) }}
              </p>
              <p class="whitespace-pre-wrap break-words">{{ message.content?.text }}</p>
            </div>
          </div>
          <p v-if="messages.length === 0" class="py-10 text-center text-xs text-neutral-400">
            暂无消息，打个招呼吧
          </p>
        </div>
        <footer class="border-t border-neutral-200 px-3 py-3 dark:border-neutral-800">
          <p v-if="error" class="mb-2 text-xs text-danger-500">{{ error }}</p>
          <div class="flex items-end gap-2">
            <textarea
              v-model="text"
              rows="1"
              placeholder="输入消息，Enter 发送"
              class="max-h-32 min-h-10 flex-1 resize-none rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-primary-500 dark:border-neutral-700"
              @keydown.enter.exact.prevent="send"
            />
            <Button :disabled="!text.trim() || sending" @click="send">
              <AppIcon name="send" class="size-4" />
            </Button>
          </div>
        </footer>
      </template>
      <div v-else class="flex flex-1 items-center justify-center text-sm text-neutral-400">
        选择左侧会话开始聊天
      </div>
    </section>

    <div
      v-if="convMenu.open && convMenu.conversation"
      class="fixed z-50 w-36 overflow-hidden rounded-[var(--radius-field)] border border-neutral-200 bg-white py-1 text-sm shadow-[var(--shadow-pop)] dark:border-neutral-700 dark:bg-neutral-900"
      :style="{ left: `${convMenu.x}px`, top: `${convMenu.y}px` }"
      @click.stop
    >
      <button class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800" @click="openConversation(convMenu.conversation!.id); closeMenus()">
        打开
      </button>
      <button class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800" @click="markConversationRead(convMenu.conversation!)">
        标记已读
      </button>
    </div>

    <div
      v-if="msgMenu.open && msgMenu.message"
      class="fixed z-50 w-36 overflow-hidden rounded-[var(--radius-field)] border border-neutral-200 bg-white py-1 text-sm shadow-[var(--shadow-pop)] dark:border-neutral-700 dark:bg-neutral-900"
      :style="{ left: `${msgMenu.x}px`, top: `${msgMenu.y}px` }"
      @click.stop
    >
      <button class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800" @click="copyMessage(msgMenu.message!)">
        复制文本
      </button>
      <button
        v-if="msgMenu.message.senderId === auth.user?.id || isGroupAdmin"
        class="block w-full px-3 py-1.5 text-left text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10"
        @click="recallMessage(msgMenu.message!)"
      >
        撤回
      </button>
    </div>

    <AppModal :open="membersOpen" title="群设置" width="max-w-lg" @close="membersOpen = false">
      <div class="space-y-4">
        <label class="flex items-center justify-between text-sm">
          <span>仅管理员/白名单可发言</span>
          <input v-model="groupSettings.onlyAdminsSpeak" type="checkbox" :disabled="!isGroupAdmin" />
        </label>
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">群公告</span>
          <textarea
            v-model="groupSettings.notice"
            rows="2"
            :disabled="!isGroupAdmin"
            class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-primary-500 disabled:opacity-60 dark:border-neutral-700"
          />
        </label>

        <div>
          <h3 class="mb-2 text-xs font-semibold text-neutral-500">成员（{{ members.length }}）</h3>
          <div class="space-y-1.5">
            <div
              v-for="member in members"
              :key="member.userId"
              class="flex flex-wrap items-center gap-2 rounded-[var(--radius-field)] border border-neutral-200 px-2.5 py-1.5 text-sm dark:border-neutral-700"
            >
              <UserAvatar :name="users[member.userId]?.nickname" :seed="member.userId" size="sm" />
              <span class="min-w-0 flex-1 truncate">
                {{ users[member.userId]?.nickname ?? member.userId }}
                <span v-if="member.role === 'owner'" class="ml-1 rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40">群主</span>
                <span v-else-if="member.role === 'admin'" class="ml-1 rounded bg-info-50 px-1 text-[10px] text-info-700 dark:bg-info-500/10">管理员</span>
                <span v-if="isMuted(member)" class="ml-1 text-[10px] text-danger-500">已禁言</span>
                <span v-else-if="!member.canSpeak" class="ml-1 text-[10px] text-warning-700">白名单</span>
              </span>
              <template v-if="isGroupAdmin && member.role !== 'owner' && member.userId !== auth.user?.id">
                <button class="text-xs text-neutral-500 hover:text-primary-600" @click="isMuted(member) ? muteMember(member, null) : muteMember(member, 60)">
                  {{ isMuted(member) ? '解除禁言' : '禁言 1 小时' }}
                </button>
                <button class="text-xs text-neutral-500 hover:text-primary-600" @click="toggleSpeak(member)">
                  {{ member.canSpeak ? '移出白名单' : '允许发言' }}
                </button>
                <button v-if="isOwner" class="text-xs text-neutral-500 hover:text-primary-600" @click="toggleAdmin(member)">
                  {{ member.role === 'admin' ? '取消管理员' : '设为管理员' }}
                </button>
                <button v-if="isOwner" class="text-xs text-neutral-500 hover:text-primary-600" @click="transferOwner(member)">转让群主</button>
              </template>
            </div>
          </div>
        </div>

        <div v-if="isOwner" class="rounded-[var(--radius-field)] border border-danger-200 p-3 dark:border-danger-500/30">
          <p class="text-xs text-neutral-500">解散群聊将删除全部消息与成员关系，不可恢复。</p>
          <Button variant="danger" size="sm" class="mt-2" @click="dissolveGroup">解散群聊</Button>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="membersOpen = false">关闭</Button>
        <Button v-if="isGroupAdmin" @click="saveGroupSettings">保存设置</Button>
      </template>
    </AppModal>

    <AppModal :open="createOpen" title="发起会话" @close="createOpen = false">
      <UserPicker v-model="pickedIds" multiple :exclude-ids="[auth.user?.id ?? '']" placeholder="搜索并选择成员（可多选）" />
      <input
        v-if="pickedIds.length > 1"
        v-model="groupName"
        placeholder="群聊名称"
        class="mt-3 w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-primary-500 dark:border-neutral-700"
      />
      <p v-if="error" class="mt-2 text-xs text-danger-500">{{ error }}</p>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="pickedIds.length === 0" @click="createConversation">
          {{ pickedIds.length > 1 ? '创建群聊' : '开始私聊' }}
        </Button>
      </template>
    </AppModal>
  </div>
</template>
