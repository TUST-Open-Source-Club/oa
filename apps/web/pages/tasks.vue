<script setup lang="ts">
/** 任务看板：多人指派、起止时间、认领、完成/终止、编辑删除、拖拽换列、项目成员管理。 */
import { Button, Input } from '@club-oa/ui'

interface ProjectItem { id: string; name: string }
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
}
interface UserItem { id: string; username: string; nickname: string; department: string | null }
interface AttachmentItem { id: string; name: string; size: number; mime: string; createdAt: string }
interface ChangeItem { id: string; action: string; actorId: string | null; createdAt: string }

const auth = useAuthStore()
const projects = ref<ProjectItem[]>([])
const projectId = ref('')
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
  if (projects.value.length > 0 && !projectId.value) projectId.value = projects.value[0].id
  if (projectId.value) await loadBoard()
}

/** 加载看板。 */
async function loadBoard() {
  const [cols, list] = await Promise.all([
    api<ColumnItem[]>(`projects/${projectId.value}/columns`),
    api<TaskItem[]>(`projects/${projectId.value}/tasks`),
  ])
  columns.value = cols
  tasks.value = list
  await resolveUsers(list.flatMap((task) => task.assigneeIds ?? []))
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
  taskOpen.value = true
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
    } else {
      await api(`projects/${projectId.value}/tasks`, { method: 'POST', body: { ...payload, columnId: draft.columnId } })
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

onMounted(loadProjects)
</script>

<template>
  <div class="space-y-5">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h1 class="text-xl font-semibold">任务</h1>
        <p class="mt-0.5 text-sm text-neutral-400">拖拽卡片流转；点击卡片上的按钮编辑、指派与流转状态</p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <select
          v-model="projectId"
          class="rounded-[var(--radius-field)] border border-neutral-300 bg-white px-3 py-2 text-sm outline-none focus:border-primary-500 dark:border-neutral-700 dark:bg-neutral-900"
          @change="loadBoard"
        >
          <option v-for="project in projects" :key="project.id" :value="project.id">{{ project.name }}</option>
        </select>
        <Input v-model="newProjectName" placeholder="新项目名称" class="max-w-40" />
        <Button variant="secondary" size="sm" @click="createProject">新建项目</Button>
        <Button v-if="projectId" variant="secondary" size="sm" @click="openMembers">成员</Button>
        <Button v-if="projectId" variant="secondary" size="sm" @click="openProjectSettings">项目设置</Button>
        <Button v-if="projectId" size="sm" @click="openCreate()">
          <AppIcon name="plus" class="size-4" /> 新建任务
        </Button>
      </div>
    </div>

    <p v-if="message" role="alert" class="text-sm text-danger-500">{{ message }}</p>

    <div v-if="columns.length" class="grid items-start gap-4 md:grid-cols-2 xl:grid-cols-3">
      <section
        v-for="column in columns"
        :key="column.id"
        class="rounded-[var(--radius-card)] border bg-neutral-100/60 p-3 transition dark:bg-neutral-900/60"
        :class="dropTarget === column.id ? 'border-primary-400 bg-primary-50/60 dark:bg-primary-900/20' : 'border-neutral-200 dark:border-neutral-800'"
        @dragover.prevent="dropTarget = column.id"
        @dragleave="dropTarget === column.id && (dropTarget = '')"
        @drop="onDrop(column.id)"
      >
        <div class="mb-2 flex items-center justify-between px-1">
          <h2 class="flex items-center gap-2 text-sm font-semibold">
            {{ column.name }}
            <span class="rounded-full bg-neutral-200 px-1.5 text-[10px] leading-4 text-neutral-500 dark:bg-neutral-800">
              {{ tasksOf(column.id).length }}
            </span>
          </h2>
          <button
            class="rounded p-1 text-neutral-400 transition hover:bg-neutral-200 hover:text-neutral-600 dark:hover:bg-neutral-800"
            aria-label="添加任务"
            @click="openCreate(column.id)"
          >
            <AppIcon name="plus" class="size-4" />
          </button>
        </div>
        <div class="space-y-2">
          <article
            v-for="task in tasksOf(column.id)"
            :key="task.id"
            draggable="true"
            class="group cursor-grab rounded-[var(--radius-field)] border border-neutral-200 bg-white p-3 shadow-sm transition hover:shadow-md active:cursor-grabbing dark:border-neutral-800 dark:bg-neutral-900"
            :class="[draggingId === task.id ? 'opacity-50' : '', task.status === 'terminated' ? 'opacity-60' : '']"
            @dragstart="draggingId = task.id"
            @dragend="draggingId = ''"
          >
            <div class="flex items-start gap-2">
              <span class="mt-1.5 size-2 shrink-0 rounded-full" :class="priorityOf(task.priority).dot" />
              <div class="min-w-0 flex-1">
                <div class="flex items-start justify-between gap-2">
                  <p class="text-sm leading-5" :class="task.status !== 'active' ? 'text-neutral-400 line-through' : ''">
                    {{ task.title }}
                  </p>
                  <span
                    v-if="statusOf(task)"
                    class="shrink-0 rounded-full px-1.5 py-0.5 text-[10px] leading-4"
                    :class="statusOf(task)?.class"
                  >
                    {{ statusOf(task)?.label }}
                  </span>
                </div>
                <p v-if="task.descriptionMd" class="mt-1 line-clamp-2 text-xs text-neutral-400">{{ task.descriptionMd }}</p>
                <div class="mt-2 flex flex-wrap items-center gap-2">
                  <div v-if="(task.assigneeIds ?? []).length" class="flex -space-x-1.5">
                    <span
                      v-for="id in task.assigneeIds.slice(0, 3)"
                      :key="id"
                      :title="users[id]?.nickname ?? id"
                    >
                      <UserAvatar :name="users[id]?.nickname" :seed="id" size="sm" class="ring-2 ring-white dark:ring-neutral-900" />
                    </span>
                    <span
                      v-if="task.assigneeIds.length > 3"
                      class="flex size-6 items-center justify-center rounded-full bg-neutral-200 text-[10px] text-neutral-600 ring-2 ring-white dark:bg-neutral-700 dark:text-neutral-200 dark:ring-neutral-900"
                    >
                      +{{ task.assigneeIds.length - 3 }}
                    </span>
                  </div>
                  <button
                    v-else
                    class="inline-flex items-center gap-1 rounded-full border border-dashed border-neutral-300 px-1.5 py-0.5 text-xs text-neutral-400 transition hover:border-primary-400 hover:text-primary-600"
                    @click="claimTask(task)"
                  >
                    <AppIcon name="users" class="size-3.5" /> 认领
                  </button>
                  <span v-if="task.startAt || task.dueAt" class="text-[11px] text-neutral-400">
                    {{ task.startAt ? new Date(task.startAt).toLocaleDateString('zh-CN') : '' }}
                    <template v-if="task.dueAt">→ {{ new Date(task.dueAt).toLocaleDateString('zh-CN') }}</template>
                  </span>
                </div>
                <div class="mt-2 flex flex-wrap gap-1 opacity-0 transition group-hover:opacity-100">
                  <button class="rounded px-1.5 py-0.5 text-xs text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800" @click="openEdit(task)">
                    编辑
                  </button>
                  <button
                    v-if="task.status === 'active'"
                    class="rounded px-1.5 py-0.5 text-xs text-success-700 hover:bg-success-50 dark:hover:bg-success-500/10"
                    @click="setStatus(task, 'done')"
                  >
                    完成
                  </button>
                  <button
                    v-if="task.status === 'active'"
                    class="rounded px-1.5 py-0.5 text-xs text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800"
                    @click="setStatus(task, 'terminated')"
                  >
                    终止
                  </button>
                  <button
                    v-if="task.status !== 'active'"
                    class="rounded px-1.5 py-0.5 text-xs text-primary-600 hover:bg-primary-50 dark:hover:bg-primary-900/30"
                    @click="setStatus(task, 'active')"
                  >
                    重开
                  </button>
                  <button class="rounded px-1.5 py-0.5 text-xs text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10" @click="removeTask(task)">
                    删除
                  </button>
                </div>
              </div>
            </div>
          </article>
          <p
            v-if="tasksOf(column.id).length === 0"
            class="rounded-[var(--radius-field)] border border-dashed border-neutral-300 py-6 text-center text-xs text-neutral-400 dark:border-neutral-700"
          >
            拖拽任务到这里
          </p>
        </div>
      </section>
    </div>

    <div
      v-else
      class="rounded-[var(--radius-card)] border border-dashed border-neutral-300 py-16 text-center text-sm text-neutral-400 dark:border-neutral-700"
    >
      创建或选择一个项目开始使用看板
    </div>

    <!-- 新建 / 编辑任务 -->
    <AppModal :open="taskOpen" :title="editingId ? '编辑任务' : '新建任务'" @close="taskOpen = false">
      <div class="space-y-4">
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">标题</span>
          <Input v-model="draft.title" placeholder="要做什么？" />
        </label>
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">内容</span>
          <textarea
            v-model="draft.descriptionMd"
            rows="4"
            placeholder="补充说明、验收标准…（支持 Markdown）"
            class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm outline-none focus:border-primary-500 dark:border-neutral-700"
          />
        </label>
        <div class="grid grid-cols-2 gap-3">
          <label class="block text-sm">
            <span class="mb-1 block text-xs text-neutral-500">所在列</span>
            <select v-model="draft.columnId" class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700">
              <option v-for="column in columns" :key="column.id" :value="column.id">{{ column.name }}</option>
            </select>
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-xs text-neutral-500">优先级</span>
            <select v-model="draft.priority" class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700">
              <option v-for="item in priorities" :key="item.value" :value="item.value">{{ item.label }}</option>
            </select>
          </label>
        </div>
        <div>
          <span class="mb-1 block text-xs text-neutral-500">负责人（可多选）</span>
          <UserPicker v-model="draft.assigneeIds" multiple placeholder="搜索并选择负责人（可留空）" />
        </div>
        <div v-if="editingId">
          <div class="mb-1 flex items-center justify-between">
            <span class="text-xs text-neutral-500">附件（{{ attachments.length }}）</span>
            <label class="cursor-pointer text-xs text-primary-600 hover:underline">
              {{ uploadingAttachment ? '上传中…' : '上传附件' }}
              <input type="file" class="hidden" :disabled="uploadingAttachment" @change="onUploadAttachment" />
            </label>
          </div>
          <ul v-if="attachments.length" class="space-y-1">
            <li
              v-for="attachment in attachments"
              :key="attachment.id"
              class="flex items-center justify-between rounded-[var(--radius-field)] border border-neutral-200 px-2.5 py-1.5 text-sm dark:border-neutral-700"
            >
              <span class="min-w-0 flex-1 truncate">{{ attachment.name }}</span>
              <span class="mx-2 shrink-0 text-xs text-neutral-400">{{ formatSize(attachment.size) }}</span>
              <button class="shrink-0 text-xs text-primary-600 hover:underline" @click="downloadAttachment(attachment)">下载</button>
              <button class="shrink-0 pl-2 text-xs text-danger-500 hover:underline" @click="removeAttachment(attachment)">删除</button>
            </li>
          </ul>
          <p v-else class="text-xs text-neutral-300">暂无附件（任意类型，单文件 ≤ 50MB）</p>
        </div>
        <div v-if="editingId">
          <span class="mb-1 block text-xs text-neutral-500">操作记录（{{ changes.length }}）</span>
          <ul v-if="changes.length" class="space-y-1">
            <li
              v-for="change in changes"
              :key="change.id"
              class="flex items-center justify-between rounded-[var(--radius-field)] border border-neutral-200 px-2.5 py-1.5 text-xs dark:border-neutral-700"
            >
              <span class="text-neutral-500">
                {{ actionLabel(change.action) }} · {{ new Date(change.createdAt).toLocaleString('zh-CN') }}
              </span>
              <button class="text-primary-600 hover:underline" @click="undoChange(change)">撤销</button>
            </li>
          </ul>
          <p v-else class="text-xs text-neutral-300">暂无操作记录</p>
        </div>
        <div class="grid grid-cols-2 gap-3">
          <label class="block text-sm">
            <span class="mb-1 block text-xs text-neutral-500">开始时间</span>
            <input v-model="draft.startAt" type="datetime-local" class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700" />
          </label>
          <label class="block text-sm">
            <span class="mb-1 block text-xs text-neutral-500">截止时间</span>
            <input v-model="draft.dueAt" type="datetime-local" class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700" />
          </label>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="taskOpen = false">取消</Button>
        <Button :disabled="!draft.title.trim() || saving" @click="saveTask">{{ saving ? '保存中…' : '保存' }}</Button>
      </template>
    </AppModal>

    <!-- 项目设置 -->
    <AppModal :open="projectOpen" title="项目设置" @close="projectOpen = false">
      <div class="space-y-3">
        <label class="block text-sm">
          <span class="mb-1 block text-xs text-neutral-500">项目名称</span>
          <Input v-model="projectNameDraft" @keyup.enter="saveProject" />
        </label>
        <div class="rounded-[var(--radius-field)] border border-danger-200 p-3 dark:border-danger-500/30">
          <p class="text-xs text-neutral-500">删除项目会连同其看板列、任务、附件一并删除，且不可恢复。</p>
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
      <p class="mb-3 text-xs text-neutral-400">添加成员后，他们才能进入该项目并认领任务。</p>
      <UserPicker v-model="memberIds" multiple :exclude-ids="[auth.user?.id ?? '']" placeholder="搜索并添加成员" />
      <template #footer>
        <Button variant="secondary" @click="membersOpen = false">关闭</Button>
        <Button :disabled="memberIds.length === 0" @click="addMembers">添加</Button>
      </template>
    </AppModal>
  </div>
</template>
