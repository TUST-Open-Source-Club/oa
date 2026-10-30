<script setup lang="ts">
/** 任务看板：多人指派、起止时间、认领、完成/终止、编辑删除、拖拽换列、项目成员管理。 */
import { Button, Input } from '@club-oa/ui'

interface ProjectItem { id: string; name: string }
interface LabelItem {
  id: string
  name: string
  color: string
}
interface ColumnItem { id: string; name: string; position: number; isDone: boolean }
interface TaskItem {
  id: string
  columnId: string
  title: string
  descriptionMd: string | null
  assigneeId: string | null
  assigneeIds: string[]
  startAt: string | null
  dueAt: string | null
  status: string
  completedAt: string | null
  priority: string
  labels?: Array<{ id: string; name: string; color: string }>
  coverAttachmentId?: string | null
  coverUrl?: string | null
  blockedBy?: Array<{ id: string; title: string; status: string }>
}
interface UserItem { id: string; username: string; nickname: string; department: string | null }
interface AttachmentItem { id: string; name: string; size: number; mime: string; createdAt: string }
interface ChangeItem { id: string; action: string; actorId: string | null; createdAt: string }

const auth = useAuthStore()
const route = useRoute()
const router = useRouter()
const projects = ref<ProjectItem[]>([])
const projectId = ref('')
const boards = ref<Array<{ id: string; name: string; position: number }>>([])
const labels = ref<LabelItem[]>([])
const draftLabelIds = ref<string[]>([])
const filters = reactive({ q: '', assigneeId: '', labelId: '', due: '' })
const archivedOpen = ref(false)
const archivedTasks = ref<TaskItem[]>([])
const quickAddTitle = ref('')
const currentCoverId = ref<string | null>(null)
const syncCursor = ref('')
let syncTimer: ReturnType<typeof setInterval> | undefined
const templates = ref<Array<{ id: string; name: string; title: string; descriptionMd: string }>>([])
const myTasksOpen = ref(false)
const myTasks = ref<TaskItem[]>([])
const comments = ref<Array<{ id: string; authorId: string; contentMd: string; createdAt: string }>>([])
const commentText = ref('')
const commentMentions = ref<Record<string, string>>({})
const boardMembersOpen = ref(false)
const boardMembers = ref<Array<{ userId: string; role: string; inherited: boolean }>>([])
const coverDraft = ref('')
const boardId = ref('')
const columns = ref<ColumnItem[]>([])
const tasks = ref<TaskItem[]>([])
const users = ref<Record<string, UserItem>>({})
const newProjectName = ref('')
const message = ref('')
const draggingId = ref('')
const dropTarget = ref('')

const priorities = [
  { value: 'low', label: '低', dot: 'bg-neutral-300' },
  { value: 'normal', label: '普通', dot: 'bg-info-500' },
  { value: 'high', label: '高', dot: 'bg-warning-500' },
  { value: 'urgent', label: '紧急', dot: 'bg-danger-500' },
]

const taskOpen = ref(false)
const editingId = ref('')
const saving = ref(false)
const draft = reactive({
  title: '',
  descriptionMd: '',
  columnId: '',
  priority: 'normal',
  assigneeIds: [] as string[],
  startAt: '',
  dueAt: '',
})
const membersOpen = ref(false)
const projectOpen = ref(false)
const projectNameDraft = ref('')
const memberIds = ref<string[]>([])
const attachments = ref<AttachmentItem[]>([])
const uploadingAttachment = ref(false)
const changes = ref<ChangeItem[]>([])

/** 带鉴权请求。 */
function api<T>(path: string, options: Record<string, unknown> = {}) {
  return $fetch<T>(`/api/task/${path}`, {
    ...options,
    headers: { ...auth.authHeaders(), ...((options.headers as Record<string, string> | undefined) ?? {}) },
  })
}

const tasksOf = (columnId: string) => tasks.value.filter((task) => task.columnId === columnId)
const priorityOf = (value: string) => priorities.find((item) => item.value === value) ?? priorities[1]

/** 状态展示。 */
function statusOf(task: TaskItem) {
  if (task.status === 'done') return { label: '已完成', class: 'bg-success-50 text-success-700 dark:bg-success-500/10' }
  if (task.status === 'terminated') return { label: '已终止', class: 'bg-neutral-100 text-neutral-500 dark:bg-neutral-800' }
  if (task.dueAt && new Date(task.dueAt) < new Date()) return { label: '已逾期', class: 'bg-danger-50 text-danger-700 dark:bg-danger-500/10' }
  return null
}

/** ISO → datetime-local。 */
function toLocalInput(value: string | null) {
  if (!value) return ''
  const date = new Date(value)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/** datetime-local → ISO。 */
function toIso(value: string) {
  return value ? new Date(value).toISOString() : undefined
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
    // 解析失败不影响看板
  }
}

/** 加载项目列表。 */
async function loadProjects() {
  projects.value = await api<ProjectItem[]>('projects')
  const first = projects.value[0]
  if (first && !projectId.value) projectId.value = first.id
  if (projectId.value) await loadBoards()
}

/** 加载项目的看板列表（保持当前选择）。 */
async function loadBoards() {
  boards.value = await api<Array<{ id: string; name: string; position: number }>>(
    `projects/${projectId.value}/boards`,
  )
  if (!boards.value.some((item) => item.id === boardId.value)) {
    boardId.value = boards.value[0]?.id ?? ''
  }
  await loadBoard()
}

/** 切换项目。 */
async function switchProject() {
  boardId.value = ''
  await loadBoards()
}

/** 加载看板（列 + 任务）。 */
async function loadBoard() {
  if (!boardId.value) {
    columns.value = []
    tasks.value = []
    return
  }
  const params = new URLSearchParams()
  if (filters.q.trim()) params.set('q', filters.q.trim())
  if (filters.assigneeId) params.set('assigneeId', filters.assigneeId)
  if (filters.labelId) params.set('labelId', filters.labelId)
  if (filters.due) params.set('due', filters.due)
  const query = params.toString() ? `?${params.toString()}` : ''
  const [cols, list] = await Promise.all([
    api<ColumnItem[]>(`boards/${boardId.value}/columns`),
    api<TaskItem[]>(`boards/${boardId.value}/tasks${query}`),
  ])
  columns.value = cols
  tasks.value = list
  await refreshCursor()
  startSyncTimer()
  labels.value = await api<LabelItem[]>(`boards/${boardId.value}/labels`).catch(() => [])
  templates.value = await api<Array<{ id: string; name: string; title: string; descriptionMd: string }>>(
    `boards/${boardId.value}/templates`,
  ).catch(() => [])
  await resolveUsers(list.flatMap((task) => task.assigneeIds ?? []))
}

/** 新建看板。 */
async function createBoard() {
  const name = window.prompt('看板名称', '新看板')
  if (!name || !name.trim() || !projectId.value) return
  const created = await api<{ id: string }>(`projects/${projectId.value}/boards`, {
    method: 'POST',
    body: { name: name.trim() },
  })
  await loadBoards()
  boardId.value = created.id
  await loadBoard()
}

/** 切换草稿里的标签选择。 */
function toggleDraftLabel(labelId: string) {
  draftLabelIds.value = draftLabelIds.value.includes(labelId)
    ? draftLabelIds.value.filter((id) => id !== labelId)
    : [...draftLabelIds.value, labelId]
}

/** 新建看板标签（管理员）。 */
async function createLabel() {
  if (!boardId.value) return
  const name = window.prompt('标签名称', '新标签')
  if (!name || !name.trim()) return
  const palette = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#3b82f6', '#8b5cf6']
  const color = palette[labels.value.length % palette.length]
  await api(`boards/${boardId.value}/labels`, {
    method: 'POST',
    body: { name: name.trim(), color },
  })
  labels.value = await api<LabelItem[]>(`boards/${boardId.value}/labels`)
}

/** 记录当前服务端时间作为增量游标。 */
async function refreshCursor() {
  if (!boardId.value) return
  const snapshot = await api<{ serverTime: string; tasks: TaskItem[] }>(
    `boards/${boardId.value}/sync`,
  )
  mergeTasks(snapshot.tasks)
  syncCursor.value = snapshot.serverTime
}

/** 把增量任务合并进列表。 */
function mergeTasks(incoming: TaskItem[]) {
  for (const task of incoming) {
    const index = tasks.value.findIndex((item) => item.id === task.id)
    if (index >= 0) tasks.value[index] = task
    else tasks.value.push(task)
  }
}

/** 增量轮询（4s），抽屉打开时暂停避免打断编辑。 */
function startSyncTimer() {
  clearInterval(syncTimer)
  syncTimer = setInterval(async () => {
    if (!boardId.value || taskOpen.value || !syncCursor.value) return
    try {
      const result = await api<{ serverTime: string; tasks: TaskItem[] }>(
        `boards/${boardId.value}/sync`,
        { params: { since: syncCursor.value } },
      )
      mergeTasks(result.tasks)
      syncCursor.value = result.serverTime
    } catch {
      // 轮询失败忽略，下次重试
    }
  }, 4000)
}

/** 给当前卡片加依赖。 */
async function addDependency(blockerId: string) {
  if (!editingId.value || !blockerId) return
  await api(`tasks/${editingId.value}/dependencies/${blockerId}`, { method: 'POST' })
  const refreshed = await api<TaskItem>(`tasks/${editingId.value}`)
  const index = tasks.value.findIndex((item) => item.id === editingId.value)
  if (index >= 0) tasks.value[index] = refreshed
  message.value = '已添加依赖'
}

/** 移除依赖。 */
async function removeDependency(blockerId: string) {
  if (!editingId.value) return
  await api(`tasks/${editingId.value}/dependencies/${blockerId}`, { method: 'DELETE' })
  const refreshed = await api<TaskItem>(`tasks/${editingId.value}`)
  const index = tasks.value.findIndex((item) => item.id === editingId.value)
  if (index >= 0) tasks.value[index] = refreshed
}

/** 依赖候选：同看板其它卡片。 */
const dependencyCandidates = computed(() =>
  tasks.value.filter((task) => task.id !== editingId.value),
)

/** 加载任务评论。 */
async function loadComments(taskId: string) {
  comments.value = await api<Array<{ id: string; authorId: string; contentMd: string; createdAt: string }>>(
    `tasks/${taskId}/comments`,
  )
  await resolveUsers(comments.value.map((row) => row.authorId))
}

/** 发表评论（可 @ 成员）。 */
async function submitComment() {
  const content = commentText.value.trim()
  if (!content || !editingId.value) return
  await api(`tasks/${editingId.value}/comments`, {
    method: 'POST',
    body: { contentMd: content, mentions: Object.keys(commentMentions.value) },
  })
  commentText.value = ''
  commentMentions.value = {}
  await loadComments(editingId.value)
}

/** 打开看板成员设置。 */
async function openBoardMembers() {
  if (!boardId.value) return
  boardMembers.value = await api<Array<{ userId: string; role: string; inherited: boolean }>>(
    `boards/${boardId.value}/members`,
  )
  await resolveUsers(boardMembers.value.map((row) => row.userId))
  boardMembersOpen.value = true
}

/** 设置看板角色覆盖。 */
async function setBoardRole(userId: string, role: string) {
  await api(`boards/${boardId.value}/members`, {
    method: 'POST',
    body: { userId, role },
  })
  await openBoardMembers()
}

/** 删除覆盖（回到继承）。 */
async function resetBoardRole(userId: string) {
  await api(`boards/${boardId.value}/members/${userId}`, { method: 'DELETE' })
  await openBoardMembers()
}

/** 用模板在列里建卡。 */
async function quickAddFromTemplate(columnId: string, templateId: string) {
  if (!templateId) return
  await api(`columns/${columnId}/tasks/from-template`, {
    method: 'POST',
    body: { templateId },
  })
  await loadBoard()
}

/** 把当前卡片存为模板。 */
async function saveAsTemplate() {
  if (!editingId.value || !boardId.value) return
  const name = window.prompt('模板名称', draft.title || '新模板')
  if (!name || !name.trim()) return
  await api(`boards/${boardId.value}/templates`, {
    method: 'POST',
    body: {
      name: name.trim(),
      title: draft.title.trim() || '未命名卡片',
      descriptionMd: draft.descriptionMd ?? '',
    },
  })
  templates.value = await api(`boards/${boardId.value}/templates`)
  message.value = '已存为模板'
}

/** 打开我的任务。 */
async function openMyTasks() {
  myTasks.value = await api<TaskItem[]>('my/tasks')
  await resolveUsers(myTasks.value.flatMap((task) => task.assigneeIds ?? []))
  myTasksOpen.value = true
}

/** 我的任务分组。 */
const myTaskGroups = computed(() => groupTasksByDue(myTasks.value))

/** 清空筛选。 */
function clearFilters() {
  filters.q = ''
  filters.assigneeId = ''
  filters.labelId = ''
  filters.due = ''
  void loadBoard()
}

/** 归档当前卡片。 */
async function archiveTask() {
  if (!editingId.value) return
  await api(`tasks/${editingId.value}/archive`, { method: 'POST' })
  taskOpen.value = false
  await loadBoard()
}

/** 打开归档列表。 */
async function openArchived() {
  if (!boardId.value) return
  archivedTasks.value = await api<TaskItem[]>(`boards/${boardId.value}/archived-tasks`)
  archivedOpen.value = true
}

/** 恢复归档卡片。 */
async function restoreTask(task: TaskItem) {
  await api(`tasks/${task.id}/restore`, { method: 'POST' })
  archivedTasks.value = await api<TaskItem[]>(`boards/${boardId.value}/archived-tasks`)
  await loadBoard()
}

/** 封面地址走同源 BFF（去掉 /file 后缀）。 */
function coverSrc(url: string) {
  return url
    .replace('/api/v1/task/attachments/', '/api/task/attachments/')
    .replace('/file?', '?')
}

/** 把附件设为封面。 */
async function setCover(attachment: AttachmentItem) {
  if (!editingId.value) return
  await api(`tasks/${editingId.value}/cover`, {
    method: 'POST',
    body: { attachmentId: attachment.id },
  })
  currentCoverId.value = attachment.id
}

/** 取消封面。 */
async function clearCover() {
  if (!editingId.value) return
  await api(`tasks/${editingId.value}/cover`, { method: 'DELETE' })
  currentCoverId.value = null
}

/** 列底快捷添加卡片。 */
async function quickAdd(columnId: string) {
  const title = quickAddTitle.value.trim()
  if (!title) return
  await api(`columns/${columnId}/tasks`, { method: 'POST', body: { title } })
  quickAddTitle.value = ''
  await loadBoard()
}

/** 关闭任务抽屉并清理 URL。 */
function closeTask() {
  taskOpen.value = false
  if (route.query.card) {
    const query = { ...route.query }
    delete query.card
    void router.replace({ query })
  }
}

/** 打开新建弹窗。 */
function openCreate(columnId?: string) {
  editingId.value = ''
  Object.assign(draft, {
    title: '',
    descriptionMd: '',
    columnId: columnId ?? columns.value[0]?.id ?? '',
    priority: 'normal',
    assigneeIds: [],
    startAt: '',
    dueAt: '',
  })
  draftLabelIds.value = []
  taskOpen.value = true
}

/** 打开编辑弹窗。 */
function openEdit(task: TaskItem) {
  editingId.value = task.id
  Object.assign(draft, {
    title: task.title,
    descriptionMd: task.descriptionMd ?? '',
    columnId: task.columnId,
    priority: task.priority,
    assigneeIds: [...(task.assigneeIds ?? [])],
    startAt: toLocalInput(task.startAt),
    dueAt: toLocalInput(task.dueAt),
  })
  attachments.value = []
  draftLabelIds.value = (task.labels ?? []).map((item) => item.id)
  currentCoverId.value = task.coverAttachmentId ?? null
  taskOpen.value = true
  void router.replace({ query: { ...route.query, card: task.id } })
  void loadAttachments(task.id).catch(() => undefined)
  void loadComments(task.id).catch(() => undefined)
}

/** 保存（新建或编辑）。 */
async function saveTask() {
  const title = draft.title.trim()
  if (!title || !draft.columnId) return
  saving.value = true
  message.value = ''
  const payload = {
    title,
    descriptionMd: draft.descriptionMd,
    priority: draft.priority,
    assigneeIds: draft.assigneeIds,
    startAt: toIso(draft.startAt) ?? null,
    dueAt: toIso(draft.dueAt) ?? null,
  }
  try {
    if (editingId.value) {
      await api(`tasks/${editingId.value}`, { method: 'PATCH', body: payload })
      const before = new Set(
        (tasks.value.find((item) => item.id === editingId.value)?.labels ?? []).map(
          (item) => item.id,
        ),
      )
      const after = new Set(draftLabelIds.value)
      for (const id of after) {
        if (!before.has(id)) await api(`tasks/${editingId.value}/labels/${id}`, { method: 'POST' })
      }
      for (const id of before) {
        if (!after.has(id)) await api(`tasks/${editingId.value}/labels/${id}`, { method: 'DELETE' })
      }
    } else {
      const created = await api<{ id: string }>(`projects/${projectId.value}/tasks`, {
        method: 'POST',
        body: { ...payload, columnId: draft.columnId },
      })
      for (const labelId of draftLabelIds.value) {
        await api(`tasks/${created.id}/labels/${labelId}`, { method: 'POST' })
      }
    }
    taskOpen.value = false
    await loadBoard()
  } catch (error) {
    message.value = apiErrorMessage(error)
  } finally {
    saving.value = false
  }
}

/** 认领任务。 */
async function claimTask(task: TaskItem) {
  await api(`tasks/${task.id}/claim`, { method: 'POST' })
  await loadBoard()
}

/** 切换状态。 */
async function setStatus(task: TaskItem, status: 'active' | 'done' | 'terminated') {
  await api(`tasks/${task.id}/status`, { method: 'POST', body: { status } })
  await loadBoard()
}

/** 删除任务。 */
async function removeTask(task: TaskItem) {
  if (!window.confirm(`确定删除任务「${task.title}」？`)) return
  await api(`tasks/${task.id}`, { method: 'DELETE' })
  await loadBoard()
}

/** 拖拽换列。 */
async function onDrop(columnId: string) {
  dropTarget.value = ''
  const task = tasks.value.find((item) => item.id === draggingId.value)
  draggingId.value = ''
  if (!task || task.columnId === columnId) return
  await api(`tasks/${task.id}/move`, { method: 'POST', body: { columnId } })
  await loadBoard()
}

/** 加载操作记录。 */
async function loadChanges(taskId: string) {
  try {
    changes.value = await api<ChangeItem[]>(`tasks/${taskId}/changes`)
  } catch {
    changes.value = []
  }
}

/** 撤销一次变更。 */
async function undoChange(change: ChangeItem) {
  await api(`changes/${change.id}/undo`, { method: 'POST' })
  await loadBoard()
  if (editingId.value) await loadChanges(editingId.value)
}

/** 动作文案。 */
function actionLabel(action: string) {
  return ({ update: '编辑', status: '状态变更', delete: '删除', undo: '撤销' } as Record<string, string>)[action] ?? action
}

/** 加载附件列表。 */
async function loadAttachments(taskId: string) {
  attachments.value = await api<AttachmentItem[]>(`tasks/${taskId}/attachments`)
}

/** 上传附件。 */
async function onUploadAttachment(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || !editingId.value) return
  uploadingAttachment.value = true
  message.value = ''
  try {
    await fetch(
      `/api/task/upload?taskId=${editingId.value}&name=${encodeURIComponent(file.name)}&mime=${encodeURIComponent(file.type || 'application/octet-stream')}`,
      { method: 'POST', body: file, headers: auth.authHeaders() },
    )
    await loadAttachments(editingId.value)
  } catch (error) {
    message.value = apiErrorMessage(error, '附件上传失败')
  } finally {
    uploadingAttachment.value = false
  }
}

/** 下载附件。 */
async function downloadAttachment(attachment: AttachmentItem) {
  const response = await fetch(
    `/api/task/download?taskId=${editingId.value}&attachmentId=${attachment.id}`,
    { headers: auth.authHeaders() },
  )
  if (!response.ok) return
  const blob = await response.blob()
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = attachment.name
  link.click()
  URL.revokeObjectURL(url)
}

/** 删除附件。 */
async function removeAttachment(attachment: AttachmentItem) {
  await api(`tasks/${editingId.value}/attachments/${attachment.id}`, { method: 'DELETE' })
  await loadAttachments(editingId.value)
}

/** 大小展示。 */
function formatSize(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / 1024 / 1024).toFixed(1)} MB`
}

/** 创建项目。 */
async function createProject() {
  const name = newProjectName.value.trim()
  if (!name) return
  try {
    const created = await api<{ id: string }>('projects', { method: 'POST', body: { name } })
    newProjectName.value = ''
    projectId.value = created.id
    await loadProjects()
  } catch (error) {
    message.value = apiErrorMessage(error)
  }
}

/** 打开项目设置。 */
function openProjectSettings() {
  projectNameDraft.value = projects.value.find((item) => item.id === projectId.value)?.name ?? ''
  projectOpen.value = true
}

/** 保存项目名称。 */
async function saveProject() {
  const name = projectNameDraft.value.trim()
  if (!name) return
  try {
    await api(`projects/${projectId.value}`, { method: 'PATCH', body: { name } })
    projectOpen.value = false
    await loadProjects()
  } catch (error) {
    message.value = apiErrorMessage(error)
  }
}

/** 删除项目（级联删除任务）。 */
async function deleteProject() {
  const name = projects.value.find((item) => item.id === projectId.value)?.name ?? ''
  if (!window.confirm(`确定删除项目「${name}」？其中所有任务都会被删除。`)) return
  try {
    await api(`projects/${projectId.value}`, { method: 'DELETE' })
    projectOpen.value = false
    projectId.value = ''
    await loadProjects()
  } catch (error) {
    message.value = apiErrorMessage(error)
  }
}

/** 打开成员管理。 */
function openMembers() {
  const ids = [...new Set(tasks.value.flatMap((task) => task.assigneeIds ?? []))]
  memberIds.value = ids.filter((id) => id !== auth.user?.id)
  membersOpen.value = true
}

/** 添加项目成员。 */
async function addMembers() {
  if (memberIds.value.length === 0) return
  await api(`projects/${projectId.value}/members`, { method: 'POST', body: { userIds: memberIds.value } })
  membersOpen.value = false
}

/** 键盘：N 新建、/ 搜索、Esc 关闭抽屉。 */
function onKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null
  const typing =
    target &&
    ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName) ||
    Boolean(target?.isContentEditable)
  if (event.key === 'Escape') {
    if (taskOpen.value) closeTask()
    return
  }
  if (typing) return
  if (event.key === 'n' || event.key === 'N') {
    event.preventDefault()
    openCreate()
  }
  if (event.key === '/') {
    event.preventDefault()
    document.querySelector<HTMLInputElement>('input[placeholder="搜索标题"]')?.focus()
  }
}

onMounted(async () => {
  await loadProjects()
  window.addEventListener('keydown', onKeydown)
  const cardId = String(route.query.card ?? '')
  if (cardId) {
    const task = tasks.value.find((item) => item.id === cardId)
    if (task) openEdit(task)
  }
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  clearInterval(syncTimer)
})
</script>

<template>
  <div class="mx-auto w-full max-w-[1280px]">
    <!-- 页头：项目切换 + 操作 -->
    <header class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex flex-wrap items-center gap-2">
        <select
          v-model="projectId"
          class="h-8 rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] font-medium text-[var(--text-strong)] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          @change="switchProject"
        >
          <option v-for="project in projects" :key="project.id" :value="project.id">{{ project.name }}</option>
        </select>
        <select
          v-if="boards.length"
          v-model="boardId"
          class="h-8 rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px] text-[var(--text-strong)] outline-none focus:border-primary-500"
          @change="loadBoard"
        >
          <option v-for="board in boards" :key="board.id" :value="board.id">
            {{ board.name }}
          </option>
        </select>
        <button
          v-if="projectId"
          class="rounded-[var(--radius-field)] p-1 text-neutral-400 hover:bg-[var(--surface-hover)] hover:text-neutral-600"
          title="新建看板"
          @click="createBoard"
        >
          <AppIcon name="plus" class="size-4" />
        </button>
        <span v-if="columns.length" class="text-[11px] text-neutral-400">
          {{ tasks.length }} 个任务 · {{ columns.length }} 列
        </span>
      </div>
      <div class="flex flex-wrap items-center gap-1.5">
        <Input v-model="newProjectName" placeholder="新项目名称" class="w-36" />
        <Button variant="secondary" size="sm" @click="createProject">新建项目</Button>
        <Button v-if="projectId" variant="ghost" size="sm" @click="openMembers">成员</Button>
        <Button v-if="projectId" variant="ghost" size="sm" @click="openProjectSettings">项目设置</Button>
        <Button v-if="projectId" size="sm" @click="openCreate()">
          <AppIcon name="plus" class="size-3.5" /> 新建任务
        </Button>
      </div>
    </header>

    <!-- 筛选 -->
    <div v-if="boardId" class="mt-3 flex flex-wrap items-center gap-2">
      <Input
        v-model="filters.q"
        placeholder="搜索标题"
        class="w-40"
        @keydown.enter="loadBoard"
      />
      <select
        v-model="filters.assigneeId"
        class="h-8 rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px] text-neutral-600"
        @change="loadBoard"
      >
        <option value="">全部负责人</option>
        <option v-for="(user, id) in users" :key="id" :value="id">{{ user.nickname }}</option>
      </select>
      <select
        v-model="filters.labelId"
        class="h-8 rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px] text-neutral-600"
        @change="loadBoard"
      >
        <option value="">全部标签</option>
        <option v-for="label in labels" :key="label.id" :value="label.id">{{ label.name }}</option>
      </select>
      <select
        v-model="filters.due"
        class="h-8 rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px] text-neutral-600"
        @change="loadBoard"
      >
        <option value="">全部时间</option>
        <option value="overdue">已逾期</option>
        <option value="today">今天到期</option>
        <option value="week">本周到期</option>
      </select>
      <Button variant="ghost" size="sm" @click="clearFilters">清除筛选</Button>
      <Button variant="ghost" size="sm" @click="openArchived">已归档</Button>
      <Button v-if="boardId" variant="ghost" size="sm" @click="openBoardMembers">看板成员</Button>
    </div>

    <p
      v-if="message"
      role="alert"
      class="mt-3 rounded-[var(--radius-field)] border border-danger-500/30 bg-danger-50 px-3 py-2 text-[12px] text-danger-700 dark:bg-danger-500/10"
    >
      {{ message }}
    </p>

    <!-- 看板：固定宽度列 + 横向滚动 -->
    <div v-if="columns.length" class="mt-4 flex items-start gap-3 overflow-x-auto pb-2">
      <section
        v-for="column in columns"
        :key="column.id"
        class="flex w-[300px] shrink-0 flex-col rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface-muted)] p-2 transition-colors"
        :class="dropTarget === column.id ? 'border-primary-400 bg-primary-50/60 dark:bg-primary-900/20' : ''"
        @dragover.prevent="dropTarget = column.id"
        @dragleave="dropTarget === column.id && (dropTarget = '')"
        @drop="onDrop(column.id)"
      >
        <div class="mb-1.5 flex items-center justify-between px-1.5 pt-0.5">
          <div class="flex items-center gap-1.5">
            <span class="text-[12px] font-medium text-[var(--text-strong)]">{{ column.name }}</span>
            <span
              class="rounded bg-[var(--surface)] px-1.5 text-[10px] leading-4 text-neutral-400 ring-1 ring-[var(--border)]"
            >
              {{ tasksOf(column.id).length }}
            </span>
          </div>
          <button
            class="rounded-[var(--radius-field)] p-1 text-neutral-400 transition hover:bg-[var(--surface-hover)] hover:text-neutral-600"
            aria-label="添加任务"
            @click="openCreate(column.id)"
          >
            <AppIcon name="plus" class="size-3.5" />
          </button>
        </div>

        <div class="space-y-1.5">
          <article
            v-for="task in tasksOf(column.id)"
            :key="task.id"
            draggable="true"
            class="group cursor-grab rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] p-2.5 shadow-[var(--shadow-card)] transition hover:border-[var(--border-strong)] active:cursor-grabbing"
            :class="[draggingId === task.id ? 'opacity-50' : '', task.status === 'terminated' ? 'opacity-60' : '']"
            @dragstart="draggingId = task.id"
            @dragend="draggingId = ''"
          >
            <img
              v-if="task.coverUrl"
              :src="coverSrc(task.coverUrl)"
              :alt="task.title"
              class="mb-2 h-24 w-full rounded-[var(--radius-field)] object-cover"
              loading="lazy"
            />
            <div v-if="(task.labels ?? []).length" class="mb-1.5 flex flex-wrap gap-1 pl-3">
              <span
                v-for="label in task.labels"
                :key="label.id"
                class="h-1.5 w-8 rounded-full"
                :style="{ backgroundColor: label.color }"
                :title="label.name"
              />
            </div>
            <div class="flex items-start gap-1.5">
              <span class="mt-[5px] size-1.5 shrink-0 rounded-full" :class="priorityOf(task.priority).dot" />
              <p
                class="min-w-0 flex-1 text-[13px] leading-5"
                :class="task.status !== 'active' ? 'text-neutral-400 line-through' : 'text-[var(--text-strong)]'"
              >
                {{ task.title }}
              </p>
              <span
                v-if="(task.blockedBy ?? []).length"
                class="shrink-0 rounded bg-warning-50 px-1.5 text-[10px] leading-4 text-warning-700 dark:bg-warning-500/15"
                :title="task.blockedBy!.map((row) => row.title).join('、')"
              >
                依赖 {{ task.blockedBy!.length }}
              </span>
              <span
                v-if="statusOf(task)"
                class="shrink-0 rounded px-1.5 text-[10px] leading-4"
                :class="statusOf(task)?.class"
              >
                {{ statusOf(task)?.label }}
              </span>
            </div>

            <p v-if="task.descriptionMd" class="mt-1 line-clamp-2 pl-3 text-[12px] leading-relaxed text-neutral-400">
              {{ task.descriptionMd }}
            </p>

            <div class="mt-2 flex flex-wrap items-center gap-2 pl-3">
              <div v-if="(task.assigneeIds ?? []).length" class="flex -space-x-1.5">
                <span v-for="id in task.assigneeIds.slice(0, 3)" :key="id" :title="users[id]?.nickname ?? id">
                  <UserAvatar
                    :name="users[id]?.nickname"
                    :seed="id"
                    size="sm"
                    class="ring-2 ring-[var(--surface)]"
                  />
                </span>
                <span
                  v-if="task.assigneeIds.length > 3"
                  class="flex size-6 items-center justify-center rounded-full bg-[var(--surface-muted)] text-[10px] text-neutral-500 ring-2 ring-[var(--surface)]"
                >
                  +{{ task.assigneeIds.length - 3 }}
                </span>
              </div>
              <button
                v-else
                class="inline-flex items-center gap-1 rounded-full border border-dashed border-[var(--border-strong)] px-1.5 text-[11px] text-neutral-400 transition hover:border-primary-400 hover:text-primary-600"
                @click="claimTask(task)"
              >
                <AppIcon name="users" class="size-3" /> 认领
              </button>
              <span v-if="task.startAt || task.dueAt" class="text-[11px] text-neutral-400">
                {{ task.startAt ? new Date(task.startAt).toLocaleDateString('zh-CN') : '' }}
                <template v-if="task.dueAt">→ {{ new Date(task.dueAt).toLocaleDateString('zh-CN') }}</template>
              </span>
            </div>

            <div class="mt-1.5 flex flex-wrap gap-0.5 pl-3 opacity-0 transition group-hover:opacity-100">
              <button class="rounded px-1.5 py-0.5 text-[11px] text-neutral-500 hover:bg-[var(--surface-hover)]" @click="openEdit(task)">
                编辑
              </button>
              <button
                v-if="task.status === 'active'"
                class="rounded px-1.5 py-0.5 text-[11px] text-success-700 hover:bg-success-50 dark:hover:bg-success-500/10"
                @click="setStatus(task, 'done')"
              >
                完成
              </button>
              <button
                v-if="task.status === 'active'"
                class="rounded px-1.5 py-0.5 text-[11px] text-neutral-500 hover:bg-[var(--surface-hover)]"
                @click="setStatus(task, 'terminated')"
              >
                终止
              </button>
              <button
                v-if="task.status !== 'active'"
                class="rounded px-1.5 py-0.5 text-[11px] text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/30"
                @click="setStatus(task, 'active')"
              >
                重开
              </button>
              <button
                class="rounded px-1.5 py-0.5 text-[11px] text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10"
                @click="removeTask(task)"
              >
                删除
              </button>
            </div>
          </article>

          <p
            v-if="tasksOf(column.id).length === 0"
            class="rounded-[var(--radius-card)] border border-dashed border-[var(--border-strong)] py-5 text-center text-[11px] text-neutral-400"
          >
            拖拽任务到这里
          </p>
        </div>

        <select
          v-if="templates.length"
          class="mt-1.5 h-7 w-full rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] px-2 text-[11px] text-neutral-500"
          value=""
          @change="quickAddFromTemplate(column.id, ($event.target as HTMLSelectElement).value); ($event.target as HTMLSelectElement).value = ''"
        >
          <option value="">从模板新建卡片…</option>
          <option v-for="template in templates" :key="template.id" :value="template.id">
            {{ template.name }}
          </option>
        </select>
        <form class="mt-1.5 px-0.5" @submit.prevent="quickAdd(column.id)">
          <input
            v-model="quickAddTitle"
            placeholder="+ 添加卡片，回车创建"
            class="h-8 w-full rounded-[var(--radius-field)] border border-transparent bg-transparent px-2 text-[12px] text-[var(--text-strong)] placeholder:text-neutral-400 hover:border-[var(--border)] hover:bg-[var(--surface)] focus:border-primary-500 focus:bg-[var(--surface)] focus:outline-none"
          />
        </form>
      </section>
    </div>

    <div
      v-else
      class="mt-4 rounded-[var(--radius-card)] border border-dashed border-[var(--border-strong)] px-6 py-16 text-center"
    >
      <p class="text-[13px] text-neutral-500">创建或选择一个项目开始使用看板</p>
      <p class="mt-1 text-[12px] text-neutral-400">项目内按列组织任务，拖拽卡片即可流转状态</p>
    </div>

    <!-- 新建 / 编辑任务（右侧抽屉） -->
    <template v-if="taskOpen">
      <div class="fixed inset-0 z-40 bg-black/20 backdrop-blur-[1px]" @click="closeTask" />
      <aside
        class="fixed top-0 right-0 z-50 flex h-full w-[460px] max-w-full flex-col border-l border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-pop)]"
      >
        <header class="flex h-11 shrink-0 items-center justify-between border-b border-[var(--border)] px-4">
          <h2 class="text-[13px] font-medium text-[var(--text-strong)]">
            {{ editingId ? '编辑任务' : '新建任务' }}
          </h2>
          <button
            class="rounded-[var(--radius-field)] p-1 text-neutral-400 hover:bg-[var(--surface-hover)] hover:text-neutral-600"
            aria-label="关闭"
            @click="closeTask"
          >
            <AppIcon name="close" class="size-4" />
          </button>
        </header>
        <div class="flex-1 space-y-3 overflow-y-auto p-4">
        <label class="block">
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">标题</span>
          <Input v-model="draft.title" placeholder="要做什么？" />
        </label>
        <label class="block">
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">内容</span>
          <textarea
            v-model="draft.descriptionMd"
            rows="4"
            placeholder="补充说明、验收标准…（支持 Markdown）"
            class="w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 py-2 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          />
        </label>
        <div class="grid grid-cols-2 gap-3">
          <label class="block">
            <span class="mb-1 block text-[11px] font-medium text-neutral-500">所在列</span>
            <select
              v-model="draft.columnId"
              class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
            >
              <option v-for="column in columns" :key="column.id" :value="column.id">{{ column.name }}</option>
            </select>
          </label>
          <label class="block">
            <span class="mb-1 block text-[11px] font-medium text-neutral-500">优先级</span>
            <select
              v-model="draft.priority"
              class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
            >
              <option v-for="item in priorities" :key="item.value" :value="item.value">{{ item.label }}</option>
            </select>
          </label>
        </div>
        <div>
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">负责人（可多选）</span>
          <UserPicker v-model="draft.assigneeIds" multiple placeholder="搜索并选择负责人（可留空）" />
        </div>

        <div>
          <div class="mb-1.5 flex items-center justify-between">
            <span class="text-[11px] font-medium text-neutral-500">标签</span>
            <button class="text-[11px] text-primary-600 hover:underline" @click="createLabel">新建标签</button>
          </div>
          <div v-if="labels.length" class="flex flex-wrap gap-1.5">
            <button
              v-for="label in labels"
              :key="label.id"
              class="flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] transition"
              :class="draftLabelIds.includes(label.id)
                ? 'border-transparent text-white'
                : 'border-[var(--border)] text-neutral-500 hover:border-[var(--border-strong)]'"
              :style="draftLabelIds.includes(label.id) ? { backgroundColor: label.color } : {}"
              @click="toggleDraftLabel(label.id)"
            >
              <span class="size-1.5 rounded-full" :style="{ backgroundColor: draftLabelIds.includes(label.id) ? '#fff' : label.color }" />
              {{ label.name }}
            </button>
          </div>
          <p v-else class="text-[11px] text-neutral-400">还没有标签，点右侧新建</p>
        </div>

        <div v-if="editingId" class="rounded-[var(--radius-field)] border border-[var(--border)] p-3">
          <div class="mb-1.5 flex items-center justify-between">
            <span class="text-[11px] font-medium text-neutral-500">附件（{{ attachments.length }}）</span>
            <label class="cursor-pointer text-[11px] font-medium text-primary-600 hover:text-primary-700">
              {{ uploadingAttachment ? '上传中…' : '上传附件' }}
              <input type="file" class="hidden" :disabled="uploadingAttachment" @change="onUploadAttachment" />
            </label>
          </div>
          <ul v-if="attachments.length" class="space-y-1">
            <li
              v-for="attachment in attachments"
              :key="attachment.id"
              class="flex items-center gap-2 rounded-[var(--radius-field)] bg-[var(--surface-muted)] px-2 py-1 text-[12px]"
            >
              <span class="min-w-0 flex-1 truncate">{{ attachment.name }}</span>
              <span class="shrink-0 text-[11px] text-neutral-400">{{ formatSize(attachment.size) }}</span>
              <button class="shrink-0 text-[11px] text-primary-600 hover:underline" @click="downloadAttachment(attachment)">下载</button>
              <button
                v-if="currentCoverId !== attachment.id"
                class="shrink-0 text-[11px] text-neutral-500 hover:text-primary-600"
                @click="setCover(attachment)"
              >设为封面</button>
              <button
                v-else
                class="shrink-0 text-[11px] text-primary-600 hover:underline"
                @click="clearCover"
              >取消封面</button>
              <button class="shrink-0 text-[11px] text-danger-500 hover:underline" @click="removeAttachment(attachment)">删除</button>
            </li>
          </ul>
          <p v-else class="text-[11px] text-neutral-400">暂无附件（任意类型，单文件 ≤ 50MB）</p>
        </div>

        <div v-if="editingId" class="rounded-[var(--radius-field)] border border-[var(--border)] p-3">
          <p class="mb-1.5 text-[11px] font-medium text-neutral-500">
            前置依赖（{{ (tasks.find((item) => item.id === editingId)?.blockedBy ?? []).length }}）
          </p>
          <ul
            v-if="(tasks.find((item) => item.id === editingId)?.blockedBy ?? []).length"
            class="mb-1.5 space-y-1"
          >
            <li
              v-for="blocker in tasks.find((item) => item.id === editingId)!.blockedBy"
              :key="blocker.id"
              class="flex items-center justify-between gap-2 text-[12px]"
            >
              <span class="min-w-0 flex-1 truncate" :class="blocker.status === 'done' ? 'text-neutral-400 line-through' : 'text-[var(--text-strong)]'">
                {{ blocker.title }}
              </span>
              <button class="text-[11px] text-neutral-400 hover:text-danger-600" @click="removeDependency(blocker.id)">移除</button>
            </li>
          </ul>
          <select
            class="h-7 w-full rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] px-2 text-[11px] text-neutral-500"
            value=""
            @change="addDependency(($event.target as HTMLSelectElement).value); ($event.target as HTMLSelectElement).value = ''"
          >
            <option value="">添加前置任务…</option>
            <option v-for="candidate in dependencyCandidates" :key="candidate.id" :value="candidate.id">
              {{ candidate.title }}
            </option>
          </select>
        </div>

        <div v-if="editingId" class="rounded-[var(--radius-field)] border border-[var(--border)] p-3">
          <p class="mb-1.5 text-[11px] font-medium text-neutral-500">评论（{{ comments.length }}）</p>
          <ul v-if="comments.length" class="mb-2 space-y-1.5">
            <li v-for="comment in comments" :key="comment.id" class="text-[12px]">
              <p class="text-[10px] text-neutral-400">
                {{ users[comment.authorId]?.nickname ?? '成员' }} · {{ new Date(comment.createdAt).toLocaleString('zh-CN') }}
              </p>
              <p class="whitespace-pre-wrap text-[var(--text-strong)]">{{ comment.contentMd }}</p>
            </li>
          </ul>
          <p v-else class="mb-2 text-[11px] text-neutral-400">暂无评论</p>
          <div v-if="Object.keys(commentMentions).length" class="mb-1 flex flex-wrap gap-1">
            <span
              v-for="(name, id) in commentMentions"
              :key="id"
              class="rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40 dark:text-primary-200"
            >@{{ name }}</span>
          </div>
          <div class="flex items-center gap-1">
            <select
              class="h-6 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] px-1 text-[10px] text-neutral-500"
              value=""
              @change="(commentMentions[($event.target as HTMLSelectElement).value] = users[($event.target as HTMLSelectElement).value]?.nickname ?? '成员'); ($event.target as HTMLSelectElement).value = ''"
            >
              <option value="">@ 提醒谁…</option>
              <option
                v-for="(user, id) in users"
                :key="id"
                :value="id"
              >{{ user.nickname }}</option>
            </select>
            <textarea
              v-model="commentText"
              rows="1"
              placeholder="写评论，Ctrl/Cmd + Enter 发送"
              class="min-h-7 flex-1 resize-none rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 py-1 text-[12px] outline-none focus:border-primary-500"
              @keydown.meta.enter.prevent="submitComment"
              @keydown.ctrl.enter.prevent="submitComment"
            />
            <Button size="sm" :disabled="!commentText.trim()" @click="submitComment">发送</Button>
          </div>
        </div>

        <div v-if="editingId" class="rounded-[var(--radius-field)] border border-[var(--border)] p-3">
          <p class="mb-1.5 text-[11px] font-medium text-neutral-500">操作记录（{{ changes.length }}）</p>
          <ul v-if="changes.length" class="space-y-1">
            <li
              v-for="change in changes"
              :key="change.id"
              class="flex items-center justify-between gap-2 text-[11px] text-neutral-500"
            >
              <span class="truncate">
                {{ actionLabel(change.action) }} · {{ new Date(change.createdAt).toLocaleString('zh-CN') }}
              </span>
              <button class="shrink-0 text-primary-600 hover:underline" @click="undoChange(change)">撤销</button>
            </li>
          </ul>
          <p v-else class="text-[11px] text-neutral-400">暂无操作记录</p>
        </div>

        <div class="grid grid-cols-2 gap-3">
          <label class="block">
            <span class="mb-1 block text-[11px] font-medium text-neutral-500">开始时间</span>
            <input
              v-model="draft.startAt"
              type="datetime-local"
              class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
            />
          </label>
          <label class="block">
            <span class="mb-1 block text-[11px] font-medium text-neutral-500">截止时间</span>
            <input
              v-model="draft.dueAt"
              type="datetime-local"
              class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
            />
          </label>
        </div>
        </div>
        <footer class="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--border)] p-3">
          <div class="flex items-center gap-2">
            <Button v-if="editingId" variant="danger" size="sm" @click="archiveTask">归档</Button>
            <Button v-if="editingId" variant="ghost" size="sm" @click="saveAsTemplate">存为模板</Button>
          </div>
          <div class="flex items-center gap-2">
            <Button variant="secondary" @click="closeTask">取消</Button>
            <Button :loading="saving" @click="saveTask">保存</Button>
          </div>
        </footer>
      </aside>
    </template>

    <!-- 归档 -->
    <AppModal :open="archivedOpen" title="已归档卡片" @close="archivedOpen = false">
      <p v-if="archivedTasks.length === 0" class="py-6 text-center text-[12px] text-neutral-400">
        没有归档卡片
      </p>
      <ul v-else class="space-y-1.5">
        <li
          v-for="task in archivedTasks"
          :key="task.id"
          class="flex items-center justify-between gap-3 rounded-[var(--radius-field)] border border-[var(--border)] px-2.5 py-2"
        >
          <span class="min-w-0 flex-1 truncate text-[13px] text-[var(--text-strong)]">{{ task.title }}</span>
          <Button variant="secondary" size="sm" @click="restoreTask(task)">恢复</Button>
        </li>
      </ul>
      <template #footer>
        <Button variant="secondary" @click="archivedOpen = false">关闭</Button>
      </template>
    </AppModal>

    <!-- 我的任务 -->
    <AppModal :open="myTasksOpen" title="我的任务" @close="myTasksOpen = false">
      <p v-if="myTasks.length === 0" class="py-6 text-center text-[12px] text-neutral-400">
        没有指派给你的未完成任务
      </p>
      <div v-else class="space-y-4">
        <section v-for="(rows, bucket) in myTaskGroups" :key="bucket">
          <template v-if="rows.length">
            <h3 class="mb-1 text-[11px] font-medium text-neutral-400">
              {{ bucket === 'overdue' ? '已逾期' : bucket === 'today' ? '今天' : bucket === 'week' ? '本周' : bucket === 'later' ? '以后' : '无截止时间' }}
              （{{ rows.length }}）
            </h3>
            <ul class="space-y-1">
              <li v-for="task in rows" :key="task.id">
                <button
                  class="flex w-full items-center justify-between gap-2 rounded-[var(--radius-field)] px-2 py-1.5 text-left text-[12px] hover:bg-[var(--surface-hover)]"
                  @click="myTasksOpen = false; openEdit(task)"
                >
                  <span class="min-w-0 flex-1 truncate text-[var(--text-strong)]">{{ task.title }}</span>
                  <span class="shrink-0 text-[10px] text-neutral-400">
                    {{ task.dueAt ? new Date(task.dueAt).toLocaleDateString('zh-CN') : '' }}
                  </span>
                </button>
              </li>
            </ul>
          </template>
        </section>
      </div>
      <template #footer>
        <Button variant="secondary" @click="myTasksOpen = false">关闭</Button>
      </template>
    </AppModal>

    <!-- 看板成员 -->
    <AppModal :open="boardMembersOpen" title="看板成员" @close="boardMembersOpen = false">
      <table class="w-full text-[12px]">
        <thead>
          <tr class="text-left text-[11px] text-neutral-400">
            <th class="pb-1 font-normal">成员</th>
            <th class="pb-1 font-normal">角色</th>
            <th class="pb-1 font-normal"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="member in boardMembers" :key="member.userId" class="border-t border-[var(--border)]">
            <td class="py-1.5">{{ users[member.userId]?.nickname ?? member.userId }}</td>
            <td class="py-1.5">
              <select
                :value="member.role"
                class="h-7 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] px-1 text-[11px]"
                @change="setBoardRole(member.userId, ($event.target as HTMLSelectElement).value)"
              >
                <option value="admin">管理员</option>
                <option value="editor">可编辑</option>
                <option value="viewer">只读</option>
              </select>
            </td>
            <td class="py-1.5 text-right">
              <button
                v-if="!member.inherited"
                class="text-[11px] text-neutral-400 hover:text-primary-600"
                @click="resetBoardRole(member.userId)"
              >恢复继承</button>
              <span v-else class="text-[10px] text-neutral-300">继承</span>
            </td>
          </tr>
        </tbody>
      </table>
      <template #footer>
        <Button variant="secondary" @click="boardMembersOpen = false">关闭</Button>
      </template>
    </AppModal>

    <!-- 项目设置 -->
    <AppModal :open="projectOpen" title="项目设置" @close="projectOpen = false">
      <div class="space-y-3">
        <label class="block">
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">项目名称</span>
          <Input v-model="projectNameDraft" @keyup.enter="saveProject" />
        </label>
        <div class="rounded-[var(--radius-field)] border border-danger-500/30 p-3">
          <p class="text-[12px] text-neutral-500">删除项目会连同其看板列、任务、附件一并删除，且不可恢复。</p>
          <Button variant="danger" size="sm" class="mt-2" @click="deleteProject">删除项目</Button>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="projectOpen = false">取消</Button>
        <Button :disabled="!projectNameDraft.trim()" @click="saveProject">保存</Button>
      </template>
    </AppModal>

    <!-- 项目成员 -->
    <AppModal :open="membersOpen" title="项目成员" @close="membersOpen = false">
      <p class="mb-3 text-[12px] text-neutral-500">添加成员后，他们才能进入该项目并认领任务。</p>
      <UserPicker v-model="memberIds" multiple :exclude-ids="[auth.user?.id ?? '']" placeholder="搜索并添加成员" />
      <template #footer>
        <Button variant="secondary" @click="membersOpen = false">关闭</Button>
        <Button :disabled="memberIds.length === 0" @click="addMembers">添加</Button>
      </template>
    </AppModal>
  </div>
</template>
