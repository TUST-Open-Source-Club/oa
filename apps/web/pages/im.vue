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
  content: { text?: string; attachments?: AttachmentItem[] }
  mentions?: string[]
  replyToId?: string | null
  threadRootId?: string | null
  threadReplyCount?: number
  reactions?: ReactionItem[]
  starred?: boolean
  status?: string
  editedAt?: string | null
  createdAt: string
}
/** 附件（上传后返回的元数据 + 签名 URL）。 */
interface AttachmentItem {
  id: string
  name: string
  mime: string
  size: number
  token: string
  url: string
}
/** 表情回应聚合。 */
interface ReactionItem {
  emoji: string
  count: number
  mine: boolean
}
/** 编辑历史版本。 */
interface RevisionRow {
  version: number
  content: { text?: string }
  editorId: string | null
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
  hasMention?: boolean
  pinned?: boolean
  notifyLevel?: string
  typingUserIds?: string[]
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
const uploading = ref(false)
const fileInputEl = ref<HTMLInputElement | null>(null)
const listEl = ref<HTMLElement | null>(null)
const composerEl = ref<HTMLTextAreaElement | null>(null)
const createOpen = ref(false)
const pickedIds = ref<string[]>([])
const groupName = ref('')
const mentionOpen = ref(false)
const mentionQuery = ref('')
const mentionIndex = ref(0)
/** 已选提及：ID → 昵称；'@all' 表示所有人。 */
const pendingMentions = ref<Record<string, string>>({})
/** 正在引用的消息。 */
const replyTo = ref<MessageItem | null>(null)
/** 附件可直接给 <img>/<a> 用的同源地址（走 BFF 转发签名）。 */
function attachmentSrc(attachment: AttachmentItem) {
  return attachment.url.replace('/api/v1/im/', '/api/im/')
}

/** 读取文件为 base64（去掉 data URL 前缀）。 */
function readBase64(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = String(reader.result ?? '')
      resolve(result.slice(result.indexOf(',') + 1))
    }
    reader.onerror = () => reject(reader.error ?? new Error('读取文件失败'))
    reader.readAsDataURL(file)
  })
}

/** 选择文件。 */
function pickFile() {
  fileInputEl.value?.click()
}

/** 上传并作为图片/文件消息发送。 */
async function onFilePicked(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || !activeId.value) return
  if (file.size === 0) {
    error.value = '文件为空'
    return
  }
  if (file.size > 4 * 1024 * 1024) {
    error.value = '文件不能超过 4MB'
    return
  }
  uploading.value = true
  error.value = ''
  try {
    const dataBase64 = await readBase64(file)
    const attachment = await api<AttachmentItem>('uploads', {
      method: 'POST',
      body: {
        name: file.name,
        mime: file.type || 'application/octet-stream',
        dataBase64,
      },
    })
    await api(`conversations/${activeId.value}/messages`, {
      method: 'POST',
      body: {
        type: file.type.startsWith('image/') ? 'image' : 'file',
        content: { attachments: [attachment] },
      },
    })
    await loadMessages()
    await loadConversations()
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    uploading.value = false
  }
}

/** 正在输入的成员昵称（不含自己）。 */
const typingUsers = computed(() => {
  const ids = active.value?.typingUserIds ?? []
  return ids
    .filter((id) => id !== auth.user?.id)
    .map((id) => users.value[id]?.nickname ?? '成员')
})
let lastTypingAt = 0
/** 搜索面板状态。 */
const searchOpen = ref(false)
const searchQuery = ref('')
const searchResults = ref<Array<MessageItem & { conversationId: string }>>([])
const searching = ref(false)
let searchTimer: ReturnType<typeof setTimeout> | undefined
/** 最近一条自己消息的已读情况。 */
const receipts = ref<{ read: number; total: number } | null>(null)
/** 快捷表情。 */
const QUICK_REACTIONS = ['👍', '🎉', '❤️', '😄', '👀']
const reactionPending = ref(false)
/** 线程面板根消息与回复。 */
const threadRoot = ref<MessageItem | null>(null)
const threadReplies = ref<MessageItem[]>([])
const threadText = ref('')
const threadLoading = ref(false)
const threadSending = ref(false)

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
const editOpen = ref(false)
const editText = ref('')
const editTarget = ref<MessageItem | null>(null)
const savingEdit = ref(false)
const historyOpen = ref(false)
const historyRows = ref<RevisionRow[]>([])
const historyLoading = ref(false)
const members = ref<MemberRow[]>([])
const groupSettings = reactive({ onlyAdminsSpeak: false, notice: '' })
const myRole = computed(() => members.value.find((row) => row.userId === auth.user?.id)?.role ?? 'member')
const isGroupAdmin = computed(() => ['owner', 'admin'].includes(myRole.value))
const isOwner = computed(() => myRole.value === 'owner')

/** 会话排序：有未读 @ 的置顶，其次按更新时间倒序。 */
const sortedConversations = computed(() =>
  [...conversations.value].sort((a, b) => {
    if (Boolean(a.pinned) !== Boolean(b.pinned)) return a.pinned ? -1 : 1
    if (Boolean(a.hasMention) !== Boolean(b.hasMention)) return a.hasMention ? -1 : 1
    return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
  }),
)

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
      ...(item.typingUserIds ?? []),
      ...(item.lastMessage?.senderId ? [item.lastMessage.senderId] : []),
    ]),
  )
  const first = conversations.value[0]
  if (!activeId.value && first) {
    await openConversation(first.id)
  }
}

/** 加载当前会话消息。 */
async function loadMessages(scroll = true) {
  if (!activeId.value) return
  messages.value = await api<MessageItem[]>(`conversations/${activeId.value}/messages?limit=100`)
  await resolveUsers(messages.value.map((item) => item.senderId ?? ''))
  await loadReceipts()
  if (scroll) {
    await nextTick()
    listEl.value?.scrollTo({ top: listEl.value.scrollHeight })
  }
}

/** 打开会话并标记已读（上报已读到的最新 seq）。 */
async function openConversation(id: string) {
  closeThread()
  receipts.value = null
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
/** @ 候选：会话成员（排除自己），群主/管理员额外可 @所有人。 */
const mentionCandidates = computed(() => {
  const current = active.value
  if (!current) return [] as Array<{ id: string; name: string }>
  const q = mentionQuery.value.trim().toLowerCase()
  const items = current.memberIds
    .filter((id) => id !== auth.user?.id)
    .map((id) => ({ id, name: users.value[id]?.nickname ?? id }))
    .filter((item) => !q || item.name.toLowerCase().includes(q))
  const canAll =
    current.type === 'group' &&
    isGroupAdmin.value &&
    (!q || '所有人'.includes(q) || 'all'.includes(q))
  return canAll ? [{ id: '@all', name: '所有人' }, ...items] : items
})

/** 插入选中的提及。 */
function applyMention(id: string, name: string) {
  const at = text.value.lastIndexOf('@')
  text.value = `${text.value.slice(0, at)}@${name} `
  pendingMentions.value[id] = name
  mentionOpen.value = false
  mentionQuery.value = ''
  mentionIndex.value = 0
}

/** 输入心跳（节流 2 秒，服务端 5 秒过期）。 */
function notifyTyping() {
  if (!activeId.value) return
  const now = Date.now()
  if (now - lastTypingAt < 2000) return
  lastTypingAt = now
  void api(`conversations/${activeId.value}/typing`, { method: 'POST' }).catch(() => undefined)
}

/** 输入时识别 @ 片段。 */
function onTextInput() {
  notifyTyping()
  const value = text.value
  const at = value.lastIndexOf('@')
  const segment = at >= 0 ? value.slice(at + 1) : ''
  const trigger = at >= 0 && !/\s/.test(segment) && segment.length <= 12
  mentionOpen.value = trigger
  mentionQuery.value = trigger ? segment : ''
  mentionIndex.value = 0
}

/** 提及候选键盘导航；返回 true 表示事件已消费。 */
function onMentionKeydown(event: KeyboardEvent): boolean {
  if (!mentionOpen.value) return false
  const list = mentionCandidates.value
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const step = event.key === 'ArrowDown' ? 1 : -1
    mentionIndex.value = (mentionIndex.value + step + list.length) % Math.max(list.length, 1)
    return true
  }
  if (event.key === 'Enter' || event.key === 'Tab') {
    const picked = list[mentionIndex.value]
    if (picked) {
      event.preventDefault()
      applyMention(picked.id, picked.name)
      return true
    }
  }
  if (event.key === 'Escape') mentionOpen.value = false
  return false
}

/** 删除文本后同步清理无效提及。 */
function syncPendingMentions() {
  for (const [id, name] of Object.entries(pendingMentions.value)) {
    const token = id === '@all' ? '@所有人' : `@${name}`
    if (!text.value.includes(token)) delete pendingMentions.value[id]
  }
}

/** 输入框键盘：先给提及面板，再走 Enter 发送。 */
function onComposerKeydown(event: KeyboardEvent) {
  if (onMentionKeydown(event)) return
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault()
    void send()
  }
}

async function send() {
  const content = text.value.trim()
  if (!content || !activeId.value || sending.value) return
  sending.value = true
  error.value = ''
  try {
    await api(`conversations/${activeId.value}/messages`, {
      method: 'POST',
      body: {
        type: 'text',
        content: { text: content },
        mentions: Object.keys(pendingMentions.value),
        replyToId: replyTo.value?.id,
      },
    })
    text.value = ''
    pendingMentions.value = {}
    replyTo.value = null
    mentionOpen.value = false
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

/** 按 ID 查找已加载的消息。 */
function messageById(id?: string | null) {
  if (!id) return null
  return messages.value.find((item) => item.id === id) ?? null
}

/** 引用块/预览用的单行摘要。 */
function quoteText(message: MessageItem | null) {
  if (!message) return '原消息不可用'
  if (message.status === 'recalled') return '该消息已撤回'
  if (message.type === 'text') return message.content?.text ?? ''
  return `[${message.type}]`
}

/** 开始引用某条消息。 */
function startReply(message: MessageItem) {
  closeMenus()
  if (message.status === 'recalled') return
  replyTo.value = message
  composerEl.value?.focus()
}

/** 跳转到被引用的消息并短暂高亮。 */
function jumpToMessage(id?: string | null) {
  if (!id) return
  const el = document.getElementById(`msg-${id}`)
  if (!el) return
  el.scrollIntoView({ block: 'center', behavior: 'smooth' })
  el.classList.add('ring-2', 'ring-primary-500/50')
  setTimeout(() => el.classList.remove('ring-2', 'ring-primary-500/50'), 1200)
}

/** 切换表情回应（服务端返回该消息最新聚合）。 */
async function toggleReaction(message: MessageItem, emoji: string) {
  if (reactionPending.value) return
  reactionPending.value = true
  error.value = ''
  try {
    const result = await api<{ reactions: ReactionItem[] }>(
      `conversations/${activeId.value}/messages/${message.id}/reactions`,
      { method: 'POST', body: { emoji } },
    )
    message.reactions = result.reactions
    closeMenus()
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    reactionPending.value = false
  }
}

/** 搜索消息（防抖 300ms，跨会话）。 */
function onSearchInput() {
  clearTimeout(searchTimer)
  const q = searchQuery.value.trim()
  if (!q) {
    searchResults.value = []
    return
  }
  searchTimer = setTimeout(() => void runSearch(), 300)
}

/** 执行搜索。 */
async function runSearch() {
  const q = searchQuery.value.trim()
  if (!q) {
    searchResults.value = []
    return
  }
  searching.value = true
  error.value = ''
  try {
    searchResults.value = await api<Array<MessageItem & { conversationId: string }>>(
      'search/messages',
      { params: { q, limit: 20 } },
    )
    await resolveUsers(searchResults.value.map((row) => row.senderId ?? ''))
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    searching.value = false
  }
}

/** 关闭搜索并清空结果。 */
function closeSearch() {
  searchOpen.value = false
  searchQuery.value = ''
  searchResults.value = []
}

/** 跳到搜索结果：必要时补拉消息窗口，再滚动高亮。 */
async function openSearchResult(hit: MessageItem & { conversationId: string }) {
  if (hit.conversationId !== activeId.value) {
    await openConversation(hit.conversationId)
  }
  closeSearch()
  if (!messages.value.some((item) => item.id === hit.id)) {
    try {
      messages.value = await api<MessageItem[]>(
        `conversations/${hit.conversationId}/messages?beforeSeq=${hit.seq + 1}&limit=100`,
      )
      await resolveUsers(messages.value.map((item) => item.senderId ?? ''))
    } catch (err) {
      error.value = apiErrorMessage(err)
      return
    }
  }
  await nextTick()
  jumpToMessage(hit.id)
}

/** 统计自己最近一条消息在群里的已读人数。 */
async function loadReceipts() {
  receipts.value = null
  if (active.value?.type !== 'group') return
  const latest = [...messages.value].reverse().find((item) => item.senderId === auth.user?.id)
  if (!latest) return
  try {
    const rows = await api<Array<{ userId: string; read: boolean }>>(
      `conversations/${activeId.value}/messages/${latest.id}/receipts`,
    )
    receipts.value = {
      read: rows.filter((row) => row.read).length,
      total: rows.length,
    }
  } catch {
    receipts.value = null
  }
}

/** 打开线程面板（线程回复定位到其根）。 */
async function openThread(message: MessageItem) {
  closeMenus()
  const root = messageById(message.threadRootId ?? message.id)
  if (!root) return
  threadRoot.value = root
  threadText.value = ''
  await loadThread()
}

/** 关闭线程面板。 */
function closeThread() {
  threadRoot.value = null
  threadReplies.value = []
  threadText.value = ''
}

/** 拉取线程回复。 */
async function loadThread() {
  const root = threadRoot.value
  if (!root) return
  threadLoading.value = true
  try {
    threadReplies.value = await api<MessageItem[]>(
      `conversations/${activeId.value}/messages/${root.id}/thread`,
    )
    await resolveUsers(threadReplies.value.map((row) => row.senderId ?? ''))
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    threadLoading.value = false
  }
}

/** 在线程里回复。 */
async function sendThread() {
  const content = threadText.value.trim()
  const root = threadRoot.value
  if (!content || !root || threadSending.value) return
  threadSending.value = true
  error.value = ''
  try {
    await api(`conversations/${activeId.value}/messages`, {
      method: 'POST',
      body: { type: 'text', content: { text: content }, threadRootId: root.id },
    })
    threadText.value = ''
    await loadThread()
    await loadMessages(false)
    await loadConversations()
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    threadSending.value = false
  }
}

/** 打开编辑弹窗（仅自己的文本消息）。 */
function openEdit(message: MessageItem) {
  closeMenus()
  if (message.senderId !== auth.user?.id || message.type !== 'text' || message.status === 'recalled') return
  editTarget.value = message
  editText.value = message.content?.text ?? ''
  editOpen.value = true
}

/** 保存编辑（服务端写版本历史并标记 editedAt）。 */
async function saveEdit() {
  const target = editTarget.value
  const content = editText.value.trim()
  if (!target || !content || savingEdit.value) return
  savingEdit.value = true
  error.value = ''
  try {
    await api(`conversations/${activeId.value}/messages/${target.id}`, {
      method: 'PATCH',
      body: { content: { text: content } },
    })
    editOpen.value = false
    await loadMessages(false)
    await loadConversations()
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    savingEdit.value = false
  }
}

/** 打开编辑历史（版本倒序，最新在前）。 */
async function openHistory(message: MessageItem) {
  closeMenus()
  historyOpen.value = true
  historyRows.value = []
  historyLoading.value = true
  error.value = ''
  try {
    historyRows.value = await api<RevisionRow[]>(
      `conversations/${activeId.value}/messages/${message.id}/revisions`,
    )
    await resolveUsers(historyRows.value.map((row) => row.editorId ?? ''))
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    historyLoading.value = false
  }
}

/** 置顶/取消置顶会话。 */
async function setPinned(conversation: ConversationItem, pinned: boolean) {
  error.value = ''
  try {
    await api(`conversations/${conversation.id}/pin`, { method: 'POST', body: { pinned } })
    conversation.pinned = pinned
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    closeMenus()
  }
}

/** 收藏/取消收藏消息。 */
async function toggleStar(message: MessageItem) {
  error.value = ''
  try {
    const result = await api<{ starred: boolean }>(
      `conversations/${activeId.value}/messages/${message.id}/star`,
      { method: 'POST' },
    )
    message.starred = result.starred
  } catch (err) {
    error.value = apiErrorMessage(err)
  } finally {
    closeMenus()
  }
}

/** 导出会话为 Markdown / JSON（带鉴权，下载 blob）。 */
async function exportConversation(conversation: ConversationItem, format: 'md' | 'json') {
  closeMenus()
  error.value = ''
  try {
    const blob = await $fetch<Blob>(`/api/im/conversations/${conversation.id}/export`, {
      params: { format },
      headers: auth.authHeaders(),
      responseType: 'blob',
    })
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `conversation-${conversation.id}.${format}`
    link.click()
    URL.revokeObjectURL(url)
  } catch (err) {
    error.value = apiErrorMessage(err)
  }
}

/** 设置会话通知级别（all / mentions / mute）。 */
async function setNotifyLevel(conversation: ConversationItem, level: string) {
  error.value = ''
  try {
    await api(`conversations/${conversation.id}/notify`, { method: 'POST', body: { level } })
    conversation.notifyLevel = level
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
  if (last.type === 'image') return '[图片]'
  if (last.type === 'file') {
    const name = last.content?.attachments?.[0]?.name
    return name ? `[文件] ${name}` : '[文件]'
  }
  return `[${last.type}]`
}

let pollTimer: ReturnType<typeof setInterval> | undefined
onMounted(async () => {
  window.addEventListener('click', closeMenus)
  await loadConversations()
  pollTimer = setInterval(() => {
    void loadConversations().catch(() => undefined)
    void loadMessages(false).catch(() => undefined)
    if (threadRoot.value) void loadThread().catch(() => undefined)
  }, 4000)
})
onUnmounted(() => {
  clearInterval(pollTimer)
  window.removeEventListener('click', closeMenus)
})
</script>

<template>
  <div
    class="mx-auto flex h-[calc(100vh-7rem)] w-full max-w-[1400px] overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
  >
    <!-- 会话列表 -->
    <aside class="flex w-64 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--surface-muted)] md:w-72">
      <div class="flex h-11 items-center justify-between px-3">
        <div class="flex items-center gap-1.5">
          <h1 class="text-[13px] font-medium text-[var(--text-strong)]">消息</h1>
          <span class="text-[11px] text-neutral-400">{{ conversations.length }}</span>
        </div>
        <div class="flex items-center gap-0.5">
          <button
            class="rounded-[var(--radius-field)] p-1 text-neutral-400 transition hover:bg-[var(--surface-hover)] hover:text-neutral-600"
            title="搜索消息"
            aria-label="搜索消息"
            @click="searchOpen = !searchOpen"
          >
            <AppIcon name="search" class="size-4" />
          </button>
          <button
            class="rounded-[var(--radius-field)] p-1 text-neutral-400 transition hover:bg-[var(--surface-hover)] hover:text-neutral-600"
            title="发起会话"
            aria-label="发起会话"
            @click="createOpen = true"
          >
            <AppIcon name="plus" class="size-4" />
          </button>
        </div>
      </div>

      <div v-if="searchOpen" class="px-2 pb-2">
        <input
          v-model="searchQuery"
          autofocus
          placeholder="搜索全部会话消息"
          class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[12px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          @input="onSearchInput"
        />
      </div>

      <div v-if="searchOpen && searchQuery.trim()" class="flex-1 overflow-y-auto px-1.5 pb-2">
        <p v-if="searching" class="px-3 py-6 text-center text-[11px] text-neutral-400">搜索中…</p>
        <p
          v-else-if="searchResults.length === 0"
          class="px-3 py-6 text-center text-[11px] text-neutral-400"
        >
          没有找到包含「{{ searchQuery.trim() }}」的消息
        </p>
        <template v-else>
          <button
            v-for="hit in searchResults"
            :key="hit.id"
            class="mb-0.5 block w-full rounded-[var(--radius-field)] px-2 py-2 text-left transition-colors hover:bg-[var(--surface-hover)]"
            @click="openSearchResult(hit)"
          >
            <span class="flex items-center justify-between gap-2 text-[11px] text-neutral-400">
              <span class="truncate">
                {{ conversations.find((item) => item.id === hit.conversationId)?.name ?? '私聊' }}
              </span>
              <span class="shrink-0">{{ timeOf(hit.createdAt) }}</span>
            </span>
            <span class="mt-0.5 block truncate text-[12px] text-[var(--text-strong)]">
              {{ hit.content?.text }}
            </span>
            <span class="mt-0.5 block text-[10px] text-neutral-400">
              {{ users[hit.senderId ?? '']?.nickname ?? '成员' }}
            </span>
          </button>
        </template>
      </div>

      <div v-else class="flex-1 overflow-y-auto px-1.5 pb-2">
        <button
          v-for="conversation in sortedConversations"
          :key="conversation.id"
          class="mb-0.5 flex w-full items-center gap-2.5 rounded-[var(--radius-field)] px-2 py-2 text-left transition-colors"
          :class="
            conversation.id === activeId
              ? 'bg-[var(--surface)] shadow-[var(--shadow-card)] ring-1 ring-black/[0.04] dark:ring-white/[0.06]'
              : 'hover:bg-[var(--surface-hover)]'
          "
          @click="openConversation(conversation.id)"
          @contextmenu.prevent="openConvMenu($event, conversation)"
        >
          <UserAvatar :name="titleOf(conversation)" :seed="conversation.id" />
          <span class="min-w-0 flex-1">
            <span class="flex items-center justify-between gap-2">
              <span class="flex min-w-0 items-center gap-1">
                <span
                  class="truncate text-[13px] text-[var(--text-strong)]"
                  :class="conversation.hasMention ? 'font-semibold' : 'font-medium'"
                >
                  {{ titleOf(conversation) }}
                </span>
                <span
                  v-if="conversation.pinned"
                  class="shrink-0 text-[10px] text-neutral-400"
                  title="已置顶"
                >置顶</span>
                <span
                  v-if="conversation.hasMention"
                  class="shrink-0 rounded bg-primary-500/15 px-1 text-[10px] font-medium text-primary-700 dark:text-primary-200"
                  title="有人@了你"
                >@</span>
              </span>
              <span class="shrink-0 text-[10px] text-neutral-400">
                {{ conversation.lastMessage ? timeOf(conversation.lastMessage.createdAt) : '' }}
              </span>
            </span>
            <span class="mt-0.5 flex items-center justify-between gap-2">
              <span
                v-if="conversation.id === activeId && typingUsers.length"
                class="truncate text-[11px] text-primary-600 dark:text-primary-300"
              >正在输入…</span>
              <span v-else class="truncate text-[11px] text-neutral-400">{{ previewOf(conversation) }}</span>
              <span
                v-if="conversation.unread > 0"
                class="shrink-0 rounded-full bg-primary-600 px-1.5 text-[10px] leading-4 font-medium text-white"
              >
                {{ conversation.unread > 99 ? '99+' : conversation.unread }}
              </span>
            </span>
          </span>
        </button>
        <p v-if="conversations.length === 0" class="px-3 py-8 text-center text-[11px] text-neutral-400">
          还没有会话，点击右上角发起
        </p>
      </div>
    </aside>

    <!-- 消息区 -->
    <section class="flex min-w-0 flex-1 flex-col">
      <template v-if="active">
        <header class="flex h-11 items-center gap-2.5 border-b border-[var(--border)] px-4">
          <UserAvatar :name="titleOf(active)" :seed="active.id" size="sm" />
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-[13px] font-medium text-[var(--text-strong)]">{{ titleOf(active) }}</h2>
            <p v-if="typingUsers.length" class="truncate text-[11px] text-primary-600 dark:text-primary-300">
              {{ typingUsers.join('、') }} 正在输入…
            </p>
            <p v-else class="text-[11px] text-neutral-400">
              {{ active.type === 'group' ? `${active.memberIds.length} 位成员` : '私聊' }}
            </p>
          </div>
          <button
            v-if="active.type === 'group'"
            class="rounded-[var(--radius-field)] p-1 text-neutral-400 transition hover:bg-[var(--surface-hover)] hover:text-neutral-600"
            title="群设置"
            @click="openGroupSettings"
          >
            <AppIcon name="settings" class="size-4" />
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
              :id="`msg-${message.id}`"
              class="max-w-[70%] rounded-[10px] px-3 py-1.5 text-[13px] transition-shadow"
              :class="
                message.senderId === auth.user?.id
                  ? 'rounded-br-[4px] bg-primary-600 text-white'
                  : 'rounded-bl-[4px] border border-[var(--border)] bg-[var(--surface-muted)] text-[var(--text-strong)]'
              "
              @contextmenu.prevent="openMsgMenu($event, message)"
            >
              <button
                v-if="message.replyToId"
                type="button"
                class="mb-1 flex w-full flex-col items-start gap-0.5 rounded-[4px] border-l-2 px-1.5 py-0.5 text-left text-[11px]"
                :class="
                  message.senderId === auth.user?.id
                    ? 'border-white/50 bg-white/10'
                    : 'border-primary-400/60 bg-black/[0.03] dark:bg-white/[0.06]'
                "
                @click.stop="jumpToMessage(message.replyToId)"
              >
                <span class="opacity-70">
                  {{ users[messageById(message.replyToId)?.senderId ?? '']?.nickname ?? '未知成员' }}
                </span>
                <span class="line-clamp-2 opacity-80">{{ quoteText(messageById(message.replyToId)) }}</span>
              </button>
              <p class="mb-0.5 flex items-center gap-1 text-[10px] opacity-60">
                {{ message.senderId === auth.user?.id ? '我' : (users[message.senderId ?? '']?.nickname ?? '系统') }}
                · {{ timeOf(message.createdAt) }}
                <span v-if="message.starred" class="not-italic" title="已收藏">★</span>
                <span
                  v-if="message.mentions?.includes('@all')"
                  class="rounded bg-white/20 px-1 text-[10px] not-italic"
                >@所有人</span>
                <span
                  v-else-if="message.mentions?.includes(auth.user?.id ?? '')"
                  class="rounded bg-white/20 px-1 text-[10px] not-italic"
                >提到了你</span>
              </p>
              <p v-if="message.status === 'recalled'" class="italic opacity-70">该消息已撤回</p>
              <template v-else>
                <div v-if="message.content?.attachments?.length" class="mb-1 space-y-1">
                  <template v-for="attachment in message.content.attachments" :key="attachment.id">
                    <img
                      v-if="attachment.mime.startsWith('image/')"
                      :src="attachmentSrc(attachment)"
                      :alt="attachment.name"
                      class="max-h-64 w-auto rounded-[6px]"
                      loading="lazy"
                    />
                    <a
                      v-else
                      :href="attachmentSrc(attachment)"
                      :download="attachment.name"
                      class="flex items-center gap-1.5 rounded-[4px] bg-black/10 px-1.5 py-1 text-[11px] underline-offset-2 hover:underline dark:bg-white/10"
                    >
                      <AppIcon name="doc" class="size-3.5 shrink-0" />
                      <span class="truncate">{{ attachment.name }}</span>
                    </a>
                  </template>
                </div>
                <p v-if="message.content?.text" class="break-words whitespace-pre-wrap">
                  {{ message.content.text }}
                </p>
              </template>
              <button
                v-if="!message.threadRootId && (message.threadReplyCount ?? 0) > 0"
                type="button"
                class="mt-1 block rounded px-1 text-[10px] font-medium"
                :class="
                  message.senderId === auth.user?.id
                    ? 'bg-white/15 text-white hover:bg-white/25'
                    : 'bg-primary-500/10 text-primary-700 hover:bg-primary-500/20 dark:text-primary-200'
                "
                @click.stop="openThread(message)"
              >
                {{ message.threadReplyCount }} 条回复
              </button>
              <button
                v-if="message.editedAt"
                type="button"
                class="mt-0.5 block text-[10px] not-italic opacity-70 underline-offset-2 hover:underline"
                title="查看编辑历史"
                @click.stop="openHistory(message)"
              >
                已编辑
              </button>
              <div v-if="message.reactions?.length" class="mt-1 flex flex-wrap gap-1">
                <button
                  v-for="reaction in message.reactions"
                  :key="reaction.emoji"
                  type="button"
                  class="flex items-center gap-0.5 rounded-full border px-1.5 text-[10px] leading-4 transition"
                  :class="
                    message.senderId === auth.user?.id
                      ? reaction.mine
                        ? 'border-white/60 bg-white/25 text-white'
                        : 'border-transparent bg-white/10 text-white/90 hover:bg-white/20'
                      : reaction.mine
                        ? 'border-primary-400/70 bg-primary-500/15 text-primary-700 dark:text-primary-200'
                        : 'border-transparent bg-black/[0.05] text-neutral-600 hover:bg-black/[0.08] dark:bg-white/[0.08] dark:text-neutral-300'
                  "
                  :title="`${reaction.emoji} ${reaction.count} 人`"
                  @click.stop="toggleReaction(message, reaction.emoji)"
                >
                  <span>{{ reaction.emoji }}</span>
                  <span>{{ reaction.count }}</span>
                </button>
              </div>
            </div>
          </div>
          <p v-if="messages.length === 0" class="py-10 text-center text-[11px] text-neutral-400">
            暂无消息，打个招呼吧
          </p>
        </div>

        <footer class="border-t border-[var(--border)] px-3 py-2.5">
          <p
            v-if="receipts && receipts.total > 0"
            class="mb-1.5 text-right text-[10px] text-neutral-400"
          >
            已读 {{ receipts.read }}/{{ receipts.total }}
          </p>
          <p v-if="error" class="mb-2 text-[11px] text-danger-500">{{ error }}</p>
          <div
            v-if="replyTo"
            class="mb-2 flex items-center gap-2 rounded-[var(--radius-field)] border-l-2 border-primary-500 bg-[var(--surface-muted)] px-2 py-1.5 text-[11px]"
          >
            <span class="min-w-0 flex-1">
              <span class="block text-neutral-500">
                回复 {{ users[replyTo.senderId ?? '']?.nickname ?? '成员' }}
              </span>
              <span class="block truncate text-[var(--text-strong)]">{{ quoteText(replyTo) }}</span>
            </span>
            <button
              type="button"
              class="shrink-0 rounded p-0.5 text-neutral-400 hover:bg-[var(--surface-hover)] hover:text-neutral-600"
              title="取消引用"
              aria-label="取消引用"
              @click="replyTo = null"
            >
              <AppIcon name="close" class="size-3.5" />
            </button>
          </div>
          <div class="relative flex items-end gap-2">
            <div
              v-if="mentionOpen && mentionCandidates.length"
              class="absolute bottom-full left-0 z-30 mb-1 max-h-48 w-56 overflow-y-auto rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] p-1 shadow-[var(--shadow-pop)]"
            >
              <button
                v-for="(item, index) in mentionCandidates"
                :key="item.id"
                type="button"
                class="flex w-full items-center gap-2 rounded-[var(--radius-field)] px-2 py-1.5 text-left text-[12px]"
                :class="index === mentionIndex ? 'bg-[var(--surface-hover)] text-[var(--text-strong)]' : 'text-neutral-600 dark:text-neutral-300'"
                @click="applyMention(item.id, item.name)"
              >
                <span class="truncate">@{{ item.name }}</span>
                <span v-if="item.id === '@all'" class="ml-auto text-[10px] text-neutral-400">群主/管理员</span>
              </button>
            </div>
            <textarea
              ref="composerEl"
              v-model="text"
              rows="1"
              placeholder="输入消息，Enter 发送"
              class="max-h-32 min-h-9 flex-1 resize-none rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 py-2 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
              @input="onTextInput(); syncPendingMentions()"
              @keydown="onComposerKeydown"
            />
            <input
              ref="fileInputEl"
              type="file"
              class="hidden"
              accept="image/*,.pdf,.doc,.docx,.xls,.xlsx,.ppt,.pptx,.txt,.zip"
              @change="onFilePicked"
            />
            <Button
              variant="secondary"
              :disabled="uploading"
              title="发送图片或文件"
              @click="pickFile"
            >
              <AppIcon name="paperclip" class="size-4" />
            </Button>
            <Button :disabled="!text.trim() || sending" @click="send">
              <AppIcon name="send" class="size-4" />
            </Button>
          </div>
        </footer>
      </template>
      <div v-else class="flex flex-1 flex-col items-center justify-center gap-1">
        <p class="text-[13px] text-neutral-500">选择左侧会话开始聊天</p>
        <p class="text-[11px] text-neutral-400">支持群聊、@ 提醒与撤回</p>
      </div>
    </section>

    <!-- 线程面板 -->
    <aside
      v-if="threadRoot"
      class="flex w-72 shrink-0 flex-col border-l border-[var(--border)] bg-[var(--surface-muted)]/40"
    >
      <header class="flex h-11 items-center gap-2 border-b border-[var(--border)] px-3">
        <AppIcon name="chat" class="size-4 text-neutral-400" />
        <h2 class="flex-1 text-[13px] font-medium text-[var(--text-strong)]">线程</h2>
        <span class="text-[11px] text-neutral-400">{{ threadReplies.length }}</span>
        <button
          type="button"
          class="rounded-[var(--radius-field)] p-1 text-neutral-400 hover:bg-[var(--surface-hover)] hover:text-neutral-600"
          title="关闭线程"
          aria-label="关闭线程"
          @click="closeThread"
        >
          <AppIcon name="close" class="size-3.5" />
        </button>
      </header>

      <div class="flex-1 space-y-3 overflow-y-auto px-3 py-3">
        <div class="rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] p-2.5">
          <p class="mb-1 text-[10px] text-neutral-400">
            {{ users[threadRoot.senderId ?? '']?.nickname ?? '系统' }}
            · {{ timeOf(threadRoot.createdAt) }}
          </p>
          <p class="break-words whitespace-pre-wrap text-[12px] text-[var(--text-strong)]">
            {{ quoteText(threadRoot) }}
          </p>
        </div>

        <p v-if="threadLoading" class="py-4 text-center text-[11px] text-neutral-400">加载中…</p>
        <p
          v-else-if="threadReplies.length === 0"
          class="py-4 text-center text-[11px] text-neutral-400"
        >
          还没有回复，开始第一条
        </p>
        <div v-for="reply in threadReplies" :key="reply.id" class="flex items-start gap-2">
          <UserAvatar
            :name="users[reply.senderId ?? '']?.nickname ?? '成员'"
            :seed="reply.senderId ?? 'system'"
            size="sm"
          />
          <div class="min-w-0 flex-1">
            <p class="text-[10px] text-neutral-400">
              {{ users[reply.senderId ?? '']?.nickname ?? '成员' }} · {{ timeOf(reply.createdAt) }}
            </p>
            <p class="break-words whitespace-pre-wrap text-[12px] text-[var(--text-strong)]">
              {{ reply.content?.text }}
            </p>
          </div>
        </div>
      </div>

      <footer class="flex items-end gap-1.5 border-t border-[var(--border)] p-2">
        <textarea
          v-model="threadText"
          rows="2"
          placeholder="回复线程，Enter 发送"
          class="min-w-0 flex-1 resize-none rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 py-1.5 text-[12px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          @keydown.enter.exact.prevent="sendThread"
        />
        <Button :disabled="!threadText.trim() || threadSending" @click="sendThread">
          <AppIcon name="send" class="size-4" />
        </Button>
      </footer>
    </aside>

    <!-- 会话右键菜单 -->
    <div
      v-if="convMenu.open && convMenu.conversation"
      class="fixed z-50 w-36 overflow-hidden rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] py-1 text-[13px] shadow-[var(--shadow-pop)]"
      :style="{ left: `${convMenu.x}px`, top: `${convMenu.y}px` }"
      @click.stop
    >
      <button class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]" @click="openConversation(convMenu.conversation!.id); closeMenus()">
        打开
      </button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="setPinned(convMenu.conversation!, !convMenu.conversation!.pinned)"
      >
        {{ convMenu.conversation!.pinned ? '取消置顶' : '置顶' }}
      </button>
      <button class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]" @click="markConversationRead(convMenu.conversation!)">
        标记已读
      </button>
      <div class="my-1 border-t border-[var(--border)]" />
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="exportConversation(convMenu.conversation!, 'md')"
      >
        导出 Markdown
      </button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="exportConversation(convMenu.conversation!, 'json')"
      >
        导出 JSON
      </button>
      <div class="my-1 border-t border-[var(--border)]" />
      <p class="px-3 pb-0.5 pt-1 text-[10px] text-neutral-400">通知</p>
      <button
        v-for="option in [
          { level: 'all', label: '全部消息' },
          { level: 'mentions', label: '仅 @我' },
          { level: 'mute', label: '静音' },
        ]"
        :key="option.level"
        class="flex w-full items-center justify-between px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="setNotifyLevel(convMenu.conversation!, option.level)"
      >
        <span>{{ option.label }}</span>
        <span v-if="(convMenu.conversation!.notifyLevel ?? 'all') === option.level" class="text-primary-600">✓</span>
      </button>
    </div>

    <!-- 消息右键菜单 -->
    <div
      v-if="msgMenu.open && msgMenu.message"
      class="fixed z-50 w-44 overflow-hidden rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] py-1 text-[13px] shadow-[var(--shadow-pop)]"
      :style="{ left: `${msgMenu.x}px`, top: `${msgMenu.y}px` }"
      @click.stop
    >
      <div
        v-if="msgMenu.message.status !== 'recalled'"
        class="flex items-center gap-0.5 border-b border-[var(--border)] px-2 pb-1.5 pt-1"
      >
        <button
          v-for="emoji in QUICK_REACTIONS"
          :key="emoji"
          type="button"
          class="rounded px-1 text-[15px] leading-6 transition hover:bg-[var(--surface-hover)]"
          :title="`回应 ${emoji}`"
          @click="toggleReaction(msgMenu.message!, emoji)"
        >
          {{ emoji }}
        </button>
      </div>
      <button
        v-if="msgMenu.message.status !== 'recalled'"
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="startReply(msgMenu.message!)"
      >
        回复
      </button>
      <button
        v-if="msgMenu.message.status !== 'recalled'"
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="openThread(msgMenu.message!)"
      >
        线程
      </button>
      <button class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]" @click="copyMessage(msgMenu.message!)">
        复制文本
      </button>
      <button
        v-if="msgMenu.message.senderId === auth.user?.id && msgMenu.message.type === 'text' && msgMenu.message.status !== 'recalled'"
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="openEdit(msgMenu.message!)"
      >
        编辑
      </button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="toggleStar(msgMenu.message!)"
      >
        {{ msgMenu.message.starred ? '取消收藏' : '收藏' }}
      </button>
      <button
        v-if="msgMenu.message.editedAt"
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="openHistory(msgMenu.message!)"
      >
        编辑历史
      </button>
      <button
        v-if="msgMenu.message.senderId === auth.user?.id || isGroupAdmin"
        class="block w-full px-3 py-1.5 text-left text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10"
        @click="recallMessage(msgMenu.message!)"
      >
        撤回
      </button>
    </div>

    <!-- 编辑消息 -->
    <AppModal :open="editOpen" title="编辑消息" width="max-w-lg" @close="editOpen = false">
      <textarea
        v-model="editText"
        rows="3"
        class="w-full resize-none rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 py-2 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
        @keydown.meta.enter.prevent="saveEdit"
        @keydown.ctrl.enter.prevent="saveEdit"
      />
      <p class="mt-1.5 text-[11px] text-neutral-400">编辑后原内容会保留在历史版本中</p>
      <template #footer>
        <Button variant="secondary" @click="editOpen = false">取消</Button>
        <Button :disabled="!editText.trim() || savingEdit" @click="saveEdit">保存</Button>
      </template>
    </AppModal>

    <!-- 编辑历史 -->
    <AppModal :open="historyOpen" title="编辑历史" width="max-w-lg" @close="historyOpen = false">
      <p v-if="historyLoading" class="py-6 text-center text-[12px] text-neutral-400">加载中…</p>
      <p v-else-if="historyRows.length === 0" class="py-6 text-center text-[12px] text-neutral-400">
        这条消息还没有历史版本
      </p>
      <ol v-else class="space-y-2">
        <li
          v-for="(row, index) in historyRows"
          :key="row.version"
          class="rounded-[var(--radius-field)] border border-[var(--border)] p-2.5"
        >
          <div class="mb-1 flex items-center justify-between gap-2 text-[11px] text-neutral-500">
            <span class="flex min-w-0 items-center gap-1">
              <span class="shrink-0">v{{ row.version }}</span>
              <span class="truncate">{{ users[row.editorId ?? '']?.nickname ?? '已注销用户' }}</span>
              <span
                v-if="index === 0"
                class="shrink-0 rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40 dark:text-primary-200"
              >当前</span>
            </span>
            <span class="shrink-0">{{ timeOf(row.createdAt) }}</span>
          </div>
          <p class="break-words whitespace-pre-wrap text-[13px] text-[var(--text-strong)]">
            {{ row.content?.text }}
          </p>
        </li>
      </ol>
      <template #footer>
        <Button variant="secondary" @click="historyOpen = false">关闭</Button>
      </template>
    </AppModal>

    <!-- 群设置 -->
    <AppModal :open="membersOpen" title="群设置" width="max-w-lg" @close="membersOpen = false">
      <div class="space-y-3">
        <label class="flex items-center justify-between rounded-[var(--radius-field)] border border-[var(--border)] px-3 py-2 text-[12px]">
          <span class="text-neutral-600 dark:text-neutral-300">仅管理员/白名单可发言</span>
          <input v-model="groupSettings.onlyAdminsSpeak" type="checkbox" :disabled="!isGroupAdmin" />
        </label>
        <label class="block">
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">群公告</span>
          <textarea
            v-model="groupSettings.notice"
            rows="2"
            :disabled="!isGroupAdmin"
            class="w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 py-2 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20 disabled:opacity-60"
          />
        </label>

        <div>
          <h3 class="mb-1.5 text-[11px] font-medium text-neutral-500">成员（{{ members.length }}）</h3>
          <div class="space-y-1">
            <div
              v-for="member in members"
              :key="member.userId"
              class="flex flex-wrap items-center gap-2 rounded-[var(--radius-field)] border border-[var(--border)] px-2.5 py-1.5 text-[12px]"
            >
              <UserAvatar :name="users[member.userId]?.nickname" :seed="member.userId" size="sm" />
              <span class="min-w-0 flex-1 truncate text-[var(--text-strong)]">
                {{ users[member.userId]?.nickname ?? member.userId }}
                <span v-if="member.role === 'owner'" class="ml-1 rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40 dark:text-primary-200">群主</span>
                <span v-else-if="member.role === 'admin'" class="ml-1 rounded bg-info-50 px-1 text-[10px] text-info-700 dark:bg-info-500/10">管理员</span>
                <span v-if="isMuted(member)" class="ml-1 text-[10px] text-danger-500">已禁言</span>
                <span v-else-if="!member.canSpeak" class="ml-1 text-[10px] text-warning-700">白名单</span>
              </span>
              <template v-if="isGroupAdmin && member.role !== 'owner' && member.userId !== auth.user?.id">
                <button class="text-[11px] text-neutral-500 hover:text-primary-600" @click="isMuted(member) ? muteMember(member, null) : muteMember(member, 60)">
                  {{ isMuted(member) ? '解除禁言' : '禁言 1 小时' }}
                </button>
                <button class="text-[11px] text-neutral-500 hover:text-primary-600" @click="toggleSpeak(member)">
                  {{ member.canSpeak ? '移出白名单' : '允许发言' }}
                </button>
                <button v-if="isOwner" class="text-[11px] text-neutral-500 hover:text-primary-600" @click="toggleAdmin(member)">
                  {{ member.role === 'admin' ? '取消管理员' : '设为管理员' }}
                </button>
                <button v-if="isOwner" class="text-[11px] text-neutral-500 hover:text-primary-600" @click="transferOwner(member)">转让群主</button>
              </template>
            </div>
          </div>
        </div>

        <div v-if="isOwner" class="rounded-[var(--radius-field)] border border-danger-500/30 p-3">
          <p class="text-[12px] text-neutral-500">解散群聊将删除全部消息与成员关系，不可恢复。</p>
          <Button variant="danger" size="sm" class="mt-2" @click="dissolveGroup">解散群聊</Button>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="membersOpen = false">关闭</Button>
        <Button v-if="isGroupAdmin" @click="saveGroupSettings">保存设置</Button>
      </template>
    </AppModal>

    <!-- 发起会话 -->
    <AppModal :open="createOpen" title="发起会话" @close="createOpen = false">
      <UserPicker v-model="pickedIds" multiple :exclude-ids="[auth.user?.id ?? '']" placeholder="搜索并选择成员（可多选）" />
      <input
        v-if="pickedIds.length > 1"
        v-model="groupName"
        placeholder="群聊名称"
        class="mt-3 h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
      />
      <p v-if="error" class="mt-2 text-[11px] text-danger-500">{{ error }}</p>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="pickedIds.length === 0" @click="createConversation">
          {{ pickedIds.length > 1 ? '创建群聊' : '开始私聊' }}
        </Button>
      </template>
    </AppModal>
  </div>
</template>
