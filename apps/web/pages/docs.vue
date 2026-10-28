<script setup lang="ts">
/** 文档：仿语雀三栏（空间/文档树 | 正文编辑 | 大纲与历史），乐观锁保存。 */
import { Button } from '@club-oa/ui'

interface SpaceItem {
  id: string
  name: string
  type: string
}
interface NodeItem {
  id: string
  parentId: string | null
  kind: 'folder' | 'page'
  title: string
  version: number
  children?: NodeItem[]
}
interface NodeDetail {
  id: string
  title: string
  version: number
  contentMd: string
}
interface VersionItem {
  version: number
  createdAt: string
}

const auth = useAuthStore()

const spaces = ref<SpaceItem[]>([])
const spaceId = ref('')
const tree = ref<NodeItem[]>([])
const current = ref<NodeDetail | null>(null)
const draft = ref('')
const versions = ref<VersionItem[]>([])
const changes = ref<Array<{ id: string; action: string; createdAt: string }>>([])
const saving = ref(false)
const errorMessage = ref('')
const savedTip = ref('')
const keyword = ref('')
const outlineOpen = ref(true)
const historyOpen = ref(false)
const editorRoot = ref<HTMLElement | null>(null)
const editorRef = ref<{ getMarkdown?: () => string; failed?: boolean } | null>(null)
const editing = ref(false)

const createOpen = ref(false)
const createKind = ref<'page' | 'folder'>('page')
const createTitle = ref('')
const createParent = ref('')

/** 带鉴权的 API 请求。 */
function api<T>(path: string, options: Record<string, unknown> = {}) {
  return $fetch<T>(`/api/doc/${path}`, {
    ...options,
    headers: { ...auth.authHeaders(), ...((options.headers as Record<string, string> | undefined) ?? {}) },
  })
}

/** 加载空间（保持当前选择）。 */
async function loadSpaces() {
  spaces.value = await api<SpaceItem[]>('spaces')
  const first = spaces.value[0]
  if (first && !spaceId.value) spaceId.value = first.id
  await loadTree()
}

/** 加载文档树。 */
async function loadTree() {
  if (spaceId.value) tree.value = await api<NodeItem[]>(`spaces/${spaceId.value}/tree`)
}

/** 删除当前知识库（仅空间管理员）。 */
async function removeSpace() {
  if (!spaceId.value) return
  const space = spaces.value.find((item) => item.id === spaceId.value)
  if (!window.confirm(`确定删除知识库「${space?.name ?? ''}」？其中所有文档都会被删除。`)) return
  try {
    await api(`spaces/${spaceId.value}`, { method: 'DELETE' })
    spaceId.value = ''
    current.value = null
    draft.value = ''
    await loadSpaces()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error, '删除失败（仅空间管理员可删除）')
  }
}

/** 切换空间。 */
async function switchSpace() {
  current.value = null
  draft.value = ''
  await loadTree()
}

/** 打开页面。 */
async function openPage(node: NodeItem) {
  if (node.kind !== 'page') return
  errorMessage.value = ''
  savedTip.value = ''
  current.value = await api<NodeDetail>(`spaces/${spaceId.value}/nodes/${node.id}`)
  draft.value = current.value.contentMd
  editing.value = false
  versions.value = await api<VersionItem[]>(`spaces/${spaceId.value}/nodes/${node.id}/versions`)
  await loadChanges(node.id)
}

/** 新建页面/文件夹。 */
async function createNode() {
  const title = createTitle.value.trim()
  if (!title || !spaceId.value) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes`, {
      method: 'POST',
      body: { kind: createKind.value, title, parentId: createParent.value || null },
    })
    createOpen.value = false
    createTitle.value = ''
    await loadTree()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 保存（乐观锁）。 */
async function save() {
  if (!current.value || saving.value) return
  if (editorRef.value && !editorRef.value.failed) {
    draft.value = editorRef.value.getMarkdown?.() ?? draft.value
  }
  saving.value = true
  errorMessage.value = ''
  try {
    const result = await api<{ version: number }>(
      `spaces/${spaceId.value}/nodes/${current.value.id}/content`,
      { method: 'PUT', body: { contentMd: draft.value, baseVersion: current.value.version } },
    )
    current.value.version = result.version
    current.value.contentMd = draft.value
    editing.value = false
    savedTip.value = `已保存 v${result.version}`
    versions.value = await api<VersionItem[]>(`spaces/${spaceId.value}/nodes/${current.value.id}/versions`)
    await loadTree()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error, '保存失败（可能已被他人修改）')
  } finally {
    saving.value = false
  }
}

/** 加载操作记录。 */
async function loadChanges(nodeId: string) {
  try {
    changes.value = await api(`spaces/${spaceId.value}/nodes/${nodeId}/changes`)
  } catch {
    changes.value = []
  }
}

/** 撤销一次正文变更。 */
async function undoChange(change: { id: string }) {
  await api(`changes/${change.id}/undo`, { method: 'POST' })
  if (current.value) {
    current.value = await api<NodeDetail>(`spaces/${spaceId.value}/nodes/${current.value.id}`)
    draft.value = current.value.contentMd
    await loadChanges(current.value.id)
    versions.value = await api<VersionItem[]>(`spaces/${spaceId.value}/nodes/${current.value.id}/versions`)
  }
}
function cancelEdit() {
  if (!current.value) return
  draft.value = current.value.contentMd
  editing.value = false
}

/** 回滚历史版本。 */
async function restore(version: number) {
  if (!current.value) return
  await api(`spaces/${spaceId.value}/nodes/${current.value.id}/versions/${version}/restore`, { method: 'POST' })
  await openPage({ id: current.value.id, parentId: null, kind: 'page', title: current.value.title, version: 0 })
}

/** 扁平化树（含深度），文件夹在前。 */
const flatTree = computed(() => {
  const rows: Array<{ node: NodeItem; depth: number }> = []
  const walk = (items: NodeItem[], depth: number) => {
    const sorted = [...items].sort((a, b) => (a.kind === b.kind ? 0 : a.kind === 'folder' ? -1 : 1))
    for (const item of sorted) {
      rows.push({ node: item, depth })
      if (item.children?.length) walk(item.children, depth + 1)
    }
  }
  walk(tree.value, 0)
  const key = keyword.value.trim().toLowerCase()
  return key ? rows.filter((row) => row.node.title.toLowerCase().includes(key)) : rows
})

/** 供“新建”选择的文件夹列表。 */
const folders = computed(() =>
  flatTree.value
    .filter((row) => row.node.kind === 'folder')
    .map((row) => ({ id: row.node.id, label: `${'　'.repeat(row.depth)}${row.node.title}` })),
)

/** 面包屑。 */
const breadcrumb = computed(() => {
  if (!current.value) return []
  const path: Array<{ id: string; title: string }> = []
  const walk = (items: NodeItem[], trail: Array<{ id: string; title: string }>): boolean => {
    for (const item of items) {
      const next = [...trail, { id: item.id, title: item.title }]
      if (item.id === current.value?.id) {
        path.push(...next)
        return true
      }
      if (item.children?.length && walk(item.children, next)) return true
    }
    return false
  }
  walk(tree.value, [])
  return path
})

/** 当前文档大纲（Markdown 标题）。 */
const outline = computed(() => {
  const items: Array<{ level: number; text: string }> = []
  for (const line of draft.value.split('\n')) {
    const match = /^(#{1,4})\s+(.+)$/.exec(line.trim())
    if (match) {
      const [, hashes, text] = match
      if (hashes && text) items.push({ level: hashes.length, text: text.slice(0, 60) })
    }
  }
  return items
})

/** 跳转到大纲标题（按编辑器内标题元素顺序）。 */
async function jumpToHeading(index: number) {
  await nextTick()
  const headings = editorRoot.value?.querySelectorAll('h1, h2, h3, h4')
  headings?.[index]?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

/** 未保存提示。 */
const dirty = computed(() => Boolean(current.value && draft.value !== current.value.contentMd))

/** Ctrl/Cmd + S 保存。 */
function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
    event.preventDefault()
    void save()
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  void loadSpaces()
})
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="mx-auto flex h-[calc(100vh-7rem)] w-full max-w-[1440px] gap-3">
    <!-- 文档树 -->
    <aside
      class="hidden w-[264px] shrink-0 flex-col overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)] md:flex"
    >
      <div class="flex items-center gap-1 border-b border-[var(--border)] px-2 py-2">
        <select
          v-model="spaceId"
          class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[13px] font-medium text-[var(--text-strong)] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          @change="switchSpace"
        >
          <option v-for="space in spaces" :key="space.id" :value="space.id">{{ space.name }}</option>
        </select>
        <button
          class="shrink-0 rounded-[var(--radius-field)] p-1.5 text-neutral-400 transition hover:bg-danger-50 hover:text-danger-500 dark:hover:bg-danger-500/10"
          title="删除知识库"
          @click="removeSpace"
        >
          <AppIcon name="logout" class="size-4" />
        </button>
      </div>

      <div class="px-2 py-2">
        <div class="relative">
          <AppIcon name="search" class="pointer-events-none absolute top-2 left-2 size-4 text-neutral-400" />
          <input
            v-model="keyword"
            placeholder="搜索文档"
            class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] pr-2 pl-7 text-[12px] outline-none focus:border-primary-500 focus:bg-[var(--surface)] focus:ring-2 focus:ring-primary-500/20"
          />
        </div>
      </div>

      <div class="flex-1 overflow-y-auto px-1.5 pb-2">
        <button
          v-for="row in flatTree"
          :key="row.node.id"
          class="group flex h-7 w-full items-center gap-1.5 rounded-[var(--radius-field)] pr-1.5 text-left text-[12.5px] transition-colors"
          :class="
            current?.id === row.node.id
              ? 'bg-[var(--surface-hover)] font-medium text-[var(--text-strong)]'
              : 'text-neutral-600 hover:bg-[var(--surface-hover)] dark:text-neutral-300'
          "
          :style="{ paddingLeft: `${row.depth * 12 + 6}px` }"
          @click="openPage(row.node)"
        >
          <AppIcon
            :name="row.node.kind === 'folder' ? 'folder' : 'doc'"
            class="size-3.5 shrink-0"
            :class="current?.id === row.node.id ? 'text-primary-600' : 'opacity-60'"
          />
          <span class="min-w-0 flex-1 truncate">{{ row.node.title }}</span>
          <span
            class="hidden rounded p-0.5 text-neutral-400 hover:text-primary-600 group-hover:block"
            role="button"
            aria-label="在此新建"
            @click.stop="createParent = row.node.id; createOpen = true"
          >
            <AppIcon name="plus" class="size-3" />
          </span>
        </button>
        <p v-if="flatTree.length === 0" class="py-8 text-center text-[11px] text-neutral-400">暂无文档</p>
      </div>

      <div class="border-t border-[var(--border)] p-2">
        <Button size="sm" variant="secondary" block @click="createParent = ''; createOpen = true">
          <AppIcon name="plus" class="size-3.5" /> 新建
        </Button>
      </div>
    </aside>

    <!-- 正文 -->
    <section
      class="flex min-w-0 flex-1 flex-col overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <template v-if="current">
        <div class="flex h-11 items-center justify-between gap-2 border-b border-[var(--border)] px-3">
          <nav class="flex min-w-0 items-center gap-1 text-[11px] text-neutral-400">
            <template v-for="(crumb, index) in breadcrumb" :key="crumb.id">
              <span v-if="index > 0" class="text-neutral-300">/</span>
              <span
                class="max-w-40 truncate"
                :class="index === breadcrumb.length - 1 ? 'text-[var(--text-strong)]' : ''"
              >
                {{ crumb.title }}
              </span>
            </template>
          </nav>
          <div class="flex shrink-0 items-center gap-1">
            <span v-if="dirty" class="mr-1 text-[11px] text-warning-700">未保存</span>
            <span v-else-if="savedTip" class="mr-1 text-[11px] text-success-700">{{ savedTip }}</span>
            <button
              class="rounded-[var(--radius-field)] p-1 transition hover:bg-[var(--surface-hover)]"
              :class="outlineOpen ? 'text-primary-600' : 'text-neutral-400'"
              aria-label="大纲"
              @click="outlineOpen = !outlineOpen"
            >
              <AppIcon name="menu" class="size-4" />
            </button>
            <button
              class="rounded-[var(--radius-field)] p-1 transition hover:bg-[var(--surface-hover)]"
              :class="historyOpen ? 'text-primary-600' : 'text-neutral-400'"
              aria-label="历史版本"
              @click="historyOpen = !historyOpen"
            >
              <AppIcon name="settings" class="size-4" />
            </button>
            <template v-if="editing">
              <Button size="sm" variant="secondary" @click="cancelEdit">取消</Button>
              <Button size="sm" :loading="saving" @click="save">保存</Button>
            </template>
            <Button v-else size="sm" @click="editing = true">编辑</Button>
          </div>
        </div>

        <p v-if="errorMessage" class="px-6 pt-3 text-[12px] text-danger-500">{{ errorMessage }}</p>

        <div class="flex min-h-0 flex-1">
          <div class="min-w-0 flex-1 overflow-y-auto">
            <div class="mx-auto max-w-[760px] px-8 py-8">
              <h1 class="text-[26px] leading-tight font-semibold tracking-[-0.02em] text-[var(--text-strong)]">
                {{ current.title }}
              </h1>
              <p class="mt-1.5 mb-6 text-[11px] text-neutral-400">v{{ current.version }} · Ctrl/Cmd + S 保存</p>
              <div ref="editorRoot">
                <MarkdownEditor
                  :key="`${current.id}-${editing ? 'edit' : 'view'}`"
                  ref="editorRef"
                  :model-value="draft"
                  :readonly="!editing"
                  @update:model-value="draft = $event"
                />
              </div>
              <details v-if="editing" class="mt-6 text-[11px] text-neutral-400">
                <summary class="cursor-pointer">纯文本回退</summary>
                <textarea
                  v-model="draft"
                  rows="10"
                  class="mt-2 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] p-3 font-mono text-[12px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
                />
              </details>
            </div>
          </div>

          <!-- 大纲 / 历史 -->
          <aside
            v-if="outlineOpen || historyOpen"
            class="hidden w-[224px] shrink-0 overflow-y-auto border-l border-[var(--border)] px-3 py-4 lg:block"
          >
            <template v-if="outlineOpen">
              <h2 class="mb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">大纲</h2>
              <button
                v-for="(item, index) in outline"
                :key="`${index}-${item.text}`"
                class="block w-full truncate rounded-[var(--radius-field)] px-1.5 py-1 text-left text-[12px] text-neutral-500 transition hover:bg-[var(--surface-hover)] hover:text-[var(--text-strong)]"
                :style="{ paddingLeft: `${(item.level - 1) * 10 + 6}px` }"
                @click="jumpToHeading(index)"
              >
                {{ item.text }}
              </button>
              <p v-if="outline.length === 0" class="text-[11px] text-neutral-400">暂无标题</p>
            </template>

            <template v-if="historyOpen">
              <h2 class="mt-5 mb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">
                操作记录
              </h2>
              <div
                v-for="change in changes"
                :key="change.id"
                class="flex items-center justify-between gap-2 py-1 text-[11px]"
              >
                <span class="min-w-0 truncate text-neutral-500">
                  {{ change.action === 'update' ? '正文更新' : change.action === 'undo' ? '撤销' : change.action }}
                  · {{ new Date(change.createdAt).toLocaleString('zh-CN') }}
                </span>
                <button class="shrink-0 text-primary-600 hover:underline" @click="undoChange(change)">撤销</button>
              </div>
              <p v-if="changes.length === 0" class="text-[11px] text-neutral-400">暂无记录</p>

              <h2 class="mt-5 mb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">
                历史版本
              </h2>
              <div
                v-for="item in versions"
                :key="item.version"
                class="flex items-center justify-between gap-2 py-1 text-[11px]"
              >
                <span class="text-neutral-500">
                  v{{ item.version }} · {{ new Date(item.createdAt).toLocaleDateString('zh-CN') }}
                </span>
                <button class="shrink-0 text-primary-600 hover:underline" @click="restore(item.version)">回滚</button>
              </div>
            </template>
          </aside>
        </div>
      </template>

      <div v-else class="flex flex-1 flex-col items-center justify-center gap-3">
        <span
          class="flex size-12 items-center justify-center rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface-muted)] text-neutral-400"
        >
          <AppIcon name="doc" class="size-6" />
        </span>
        <p class="text-[13px] text-neutral-500">从左侧选择文档，或新建一篇开始写作</p>
        <Button size="sm" variant="secondary" @click="createParent = ''; createOpen = true">新建文档</Button>
      </div>
    </section>

    <!-- 新建 -->
    <AppModal :open="createOpen" title="新建" @close="createOpen = false">
      <div class="space-y-3">
        <div class="grid grid-cols-2 gap-1 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] p-1">
          <button
            v-for="option in [{ value: 'page', label: '文档' }, { value: 'folder', label: '文件夹' }]"
            :key="option.value"
            class="rounded-[calc(var(--radius-field)-2px)] px-3 py-1.5 text-[12px] font-medium transition"
            :class="
              createKind === option.value
                ? 'bg-[var(--surface)] text-[var(--text-strong)] shadow-[var(--shadow-card)]'
                : 'text-neutral-500 hover:text-neutral-700 dark:hover:text-neutral-200'
            "
            @click="createKind = option.value as 'page' | 'folder'"
          >
            {{ option.label }}
          </button>
        </div>
        <input
          v-model="createTitle"
          placeholder="标题"
          class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          @keyup.enter="createNode"
        />
        <label class="block">
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">所属文件夹（可选）</span>
          <select
            v-model="createParent"
            class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
          >
            <option value="">根目录</option>
            <option v-for="folder in folders" :key="folder.id" :value="folder.id">{{ folder.label }}</option>
          </select>
        </label>
      </div>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="!createTitle.trim()" @click="createNode">创建</Button>
      </template>
    </AppModal>
  </div>
</template>
