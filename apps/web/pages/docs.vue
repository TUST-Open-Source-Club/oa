<script setup lang="ts">
/** 文档：仿语雀三栏（空间/文档树 | 正文编辑 | 大纲与历史），乐观锁保存。 */
import { Button } from '@club-oa/ui'
import { diffLines } from '~/utils/text-diff'

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
  position?: number
  children?: NodeItem[]
}
interface NodeDetail {
  id: string
  title: string
  version: number
  contentMd: string
  visibility?: string
  viewCount?: number
}
interface TemplateItem {
  id: string
  name: string
  contentMd: string
  createdAt: string
}
interface RecentItem {
  id: string
  spaceId: string
  spaceName: string
  title: string
  viewedAt: string
}
interface MemberItem {
  userId: string
  role: string
  joinedAt: string
}
interface CommentItem {
  id: string
  parentId: string | null
  authorId: string
  anchor: { heading?: string } | null
  body: string
  resolvedAt: string | null
  createdAt: string
}
interface SearchHit {
  id: string
  spaceId: string
  spaceName: string
  title: string
  snippet: string
  updatedAt: string
}
interface ShareInfo {
  token: string | null
  url: string | null
  hasPassword: boolean
  expiresAt: string | null
  permission: string | null
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

const treeMenu = reactive<{
  open: boolean
  x: number
  y: number
  node: NodeItem | null
}>({ open: false, x: 0, y: 0, node: null })
const renameOpen = ref(false)
const renameTitle = ref('')
const renameTarget = ref<NodeItem | null>(null)
const moveOpen = ref(false)
const moveTarget = ref<NodeItem | null>(null)
const moveParent = ref('')

const selectedVersion = ref<number | null>(null)
const versionContent = ref('')
const versionLoading = ref(false)

const spaceSettingsOpen = ref(false)
const spaceMembers = ref<MemberItem[]>([])
const spaceUsers = ref<Record<string, { nickname: string }>>({})
const inviteIds = ref<string[]>([])
const inviteRole = ref('viewer')
const spaceError = ref('')

const following = ref(false)
const recentViews = ref<RecentItem[]>([])
const templates = ref<TemplateItem[]>([])
const createTemplateId = ref('')
const commentsOpen = ref(false)
const comments = ref<CommentItem[]>([])
const commentsLoading = ref(false)
const commentBody = ref('')
const commentAnchor = ref('')
const replyTo = ref<CommentItem | null>(null)
const commentUsers = ref<Record<string, { nickname: string }>>({})

const moreOpen = ref(false)
const importInputEl = ref<HTMLInputElement | null>(null)
const searchResults = ref<SearchHit[]>([])
const searching = ref(false)
let searchTimer: ReturnType<typeof setTimeout> | undefined

const shareOpen = ref(false)
const shareInfo = ref<ShareInfo | null>(null)
const sharePassword = ref('')
const shareExpiry = ref('')
const shareSaving = ref(false)
const shareCopied = ref(false)
const visibility = ref<'space' | 'private'>('space')

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

/** 加载最近浏览。 */
async function loadRecent() {
  try {
    recentViews.value = await api<RecentItem[]>('recent', { params: { limit: 6 } })
  } catch {
    recentViews.value = []
  }
}

/** 打开最近浏览项（必要时切换空间）。 */
async function openRecent(item: RecentItem) {
  if (item.spaceId !== spaceId.value) {
    spaceId.value = item.spaceId
    await loadTree()
  }
  await openPage({ id: item.id, parentId: null, kind: 'page', title: item.title, version: 1 })
}

/** 把当前文档正文存为空间模板。 */
async function saveAsTemplate() {
  moreOpen.value = false
  if (!current.value) return
  const name = window.prompt('模板名称', `${current.value.title} 模板`)
  if (!name || !name.trim()) return
  errorMessage.value = ''
  try {
    const content = editorRef.value?.getMarkdown?.() ?? draft.value
    await api(`spaces/${spaceId.value}/templates`, {
      method: 'POST',
      body: { name: name.trim(), contentMd: content },
    })
    savedTip.value = '已存为模板'
  } catch (error) {
    errorMessage.value = apiErrorMessage(error, '存为模板失败')
  }
}

/** 关注/取消关注当前文档。 */
async function toggleFollow() {
  if (!current.value) return
  try {
    const result = await api<{ following: boolean }>(
      `spaces/${spaceId.value}/nodes/${current.value.id}/follow`,
      { method: 'POST' },
    )
    following.value = result.following
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 打开空间设置（成员管理）。 */
async function openSpaceSettings() {
  if (!spaceId.value) return
  spaceSettingsOpen.value = true
  spaceError.value = ''
  inviteIds.value = []
  await loadSpaceMembers()
}

/** 拉取空间成员与昵称。 */
async function loadSpaceMembers() {
  if (!spaceId.value) return
  try {
    spaceMembers.value = await api<MemberItem[]>(`spaces/${spaceId.value}/members`)
    const ids = spaceMembers.value.map((row) => row.userId)
    if (ids.length) {
      try {
        const users = await $fetch<Array<{ id: string; nickname: string }>>('/api/auth/users', {
          params: { ids: ids.join(',') },
          headers: auth.authHeaders(),
        })
        for (const user of users) spaceUsers.value[user.id] = { nickname: user.nickname }
      } catch {
        // 昵称解析失败不影响成员展示
      }
    }
  } catch (error) {
    spaceError.value = apiErrorMessage(error)
  }
}

/** 我的角色。 */
const mySpaceRole = computed(
  () => spaceMembers.value.find((row) => row.userId === auth.user?.id)?.role ?? '',
)

/** 邀请成员。 */
async function inviteMembers() {
  if (!spaceId.value || inviteIds.value.length === 0) return
  spaceError.value = ''
  try {
    for (const userId of inviteIds.value) {
      await api(`spaces/${spaceId.value}/members`, {
        method: 'POST',
        body: { userId, role: inviteRole.value },
      })
    }
    inviteIds.value = []
    await loadSpaceMembers()
  } catch (error) {
    spaceError.value = apiErrorMessage(error)
  }
}

/** 调整成员角色。 */
async function changeMemberRole(member: MemberItem, role: string) {
  spaceError.value = ''
  try {
    await api(`spaces/${spaceId.value}/members/${member.userId}`, {
      method: 'PATCH',
      body: { role },
    })
    await loadSpaceMembers()
  } catch (error) {
    spaceError.value = apiErrorMessage(error)
    await loadSpaceMembers()
  }
}

/** 移除成员。 */
async function removeSpaceMember(member: MemberItem) {
  if (!window.confirm(`移除成员 ${spaceUsers.value[member.userId]?.nickname ?? member.userId}？`)) return
  spaceError.value = ''
  try {
    await api(`spaces/${spaceId.value}/members/${member.userId}`, { method: 'DELETE' })
    await loadSpaceMembers()
  } catch (error) {
    spaceError.value = apiErrorMessage(error)
  }
}

/** 转移所有权。 */
async function transferSpace(member: MemberItem) {
  if (!window.confirm('转移所有权后你会降为管理员，确定？')) return
  spaceError.value = ''
  try {
    await api(`spaces/${spaceId.value}/transfer`, {
      method: 'POST',
      body: { userId: member.userId },
    })
    await loadSpaceMembers()
  } catch (error) {
    spaceError.value = apiErrorMessage(error)
  }
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
  void loadRecent()
  following.value = await api<{ following: boolean }>(
    `spaces/${spaceId.value}/nodes/${node.id}/follow`,
  )
    .then((result) => result.following)
    .catch(() => false)
  if (commentsOpen.value) await loadComments()
}

/** 打开树节点右键菜单。 */
function openTreeMenu(event: MouseEvent, node: NodeItem) {
  treeMenu.open = true
  treeMenu.x = event.clientX
  treeMenu.y = event.clientY
  treeMenu.node = node
}

/** 关闭右键菜单。 */
function closeTreeMenu() {
  treeMenu.open = false
}

/** 开始重命名。 */
function startRename(node: NodeItem) {
  closeTreeMenu()
  renameTarget.value = node
  renameTitle.value = node.title
  renameOpen.value = true
}

/** 提交重命名。 */
async function submitRename() {
  const target = renameTarget.value
  const title = renameTitle.value.trim()
  if (!target || !title) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes/${target.id}`, {
      method: 'PATCH',
      body: { title },
    })
    renameOpen.value = false
    await loadTree()
    if (current.value?.id === target.id) current.value.title = title
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 开始移动。 */
function startMove(node: NodeItem) {
  closeTreeMenu()
  moveTarget.value = node
  moveParent.value = node.parentId ?? ''
  moveOpen.value = true
}

/** 提交移动。 */
async function submitMove() {
  const target = moveTarget.value
  if (!target) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes/${target.id}`, {
      method: 'PATCH',
      body: { parentId: moveParent.value || null },
    })
    moveOpen.value = false
    await loadTree()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 同级上移 / 下移（交换 position）。 */
async function moveSibling(node: NodeItem, direction: -1 | 1) {
  closeTreeMenu()
  const siblings = siblingsOf(node)
  const index = siblings.findIndex((item) => item.id === node.id)
  const other = siblings[index + direction]
  if (!other) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes/${node.id}`, {
      method: 'PATCH',
      body: { position: other.position ?? index + direction },
    })
    await api(`spaces/${spaceId.value}/nodes/${other.id}`, {
      method: 'PATCH',
      body: { position: node.position ?? index },
    })
    await loadTree()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 取某节点的同级列表（树顺序）。 */
function siblingsOf(node: NodeItem) {
  const walk = (items: NodeItem[]): NodeItem[] | null => {
    if (items.some((item) => item.id === node.id)) return items
    for (const item of items) {
      if (item.children?.length) {
        const hit = walk(item.children)
        if (hit) return hit
      }
    }
    return null
  }
  return walk(tree.value) ?? []
}

/** 删除节点（软删整棵子树）。 */
async function deleteNode(node: NodeItem) {
  closeTreeMenu()
  if (!window.confirm(`删除「${node.title}」及其子文档？`)) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes/${node.id}`, { method: 'DELETE' })
    if (current.value?.id === node.id) {
      current.value = null
      draft.value = ''
    }
    await loadTree()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 搜索输入防抖。 */
function onKeywordInput() {
  clearTimeout(searchTimer)
  const q = keyword.value.trim()
  if (!q) {
    searchResults.value = []
    return
  }
  searchTimer = setTimeout(() => void runSearch(), 300)
}

/** 调用服务端搜索（标题 + 正文）。 */
async function runSearch() {
  const q = keyword.value.trim()
  if (!q) {
    searchResults.value = []
    return
  }
  searching.value = true
  try {
    searchResults.value = await api<SearchHit[]>('search', { params: { q, limit: 20 } })
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  } finally {
    searching.value = false
  }
}

/** 打开搜索结果（必要时先切换空间）。 */
async function openSearchHit(hit: SearchHit) {
  if (hit.spaceId !== spaceId.value) {
    spaceId.value = hit.spaceId
    await loadTree()
  }
  await openPage({ id: hit.id, parentId: null, kind: 'page', title: hit.title, version: 0 })
  keyword.value = ''
  searchResults.value = []
}

/** 导出当前文档（md / html）。 */
async function exportDoc(format: 'md' | 'html') {
  moreOpen.value = false
  if (!current.value) return
  errorMessage.value = ''
  try {
    const blob = await $fetch<Blob>(
      `/api/doc/spaces/${spaceId.value}/nodes/${current.value.id}/export`,
      { params: { format }, headers: auth.authHeaders(), responseType: 'blob' },
    )
    const url = URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `${current.value.title}.${format}`
    link.click()
    URL.revokeObjectURL(url)
  } catch (error) {
    errorMessage.value = apiErrorMessage(error, '导出失败')
  }
}

/** 选择 Markdown 文件导入。 */
function pickImport() {
  moreOpen.value = false
  importInputEl.value?.click()
}

/** 导入 Markdown 为页面并打开。 */
async function onImportPicked(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file || !spaceId.value) return
  errorMessage.value = ''
  try {
    const contentMd = await file.text()
    const title = file.name.replace(/\.(md|markdown|txt)$/i, '')
    const created = await api<{ id: string; title: string }>(
      `spaces/${spaceId.value}/import`,
      { method: 'POST', body: { title, contentMd } },
    )
    await loadTree()
    await openPage({
      id: created.id,
      parentId: null,
      kind: 'page',
      title: created.title,
      version: 1,
    })
  } catch (error) {
    errorMessage.value = apiErrorMessage(error, '导入失败')
  }
}

/** 打开某个历史版本并与当前正文对比。 */
async function openVersion(item: VersionItem) {
  if (!current.value) return
  selectedVersion.value = item.version
  versionLoading.value = true
  errorMessage.value = ''
  try {
    const data = await api<{ version: number; contentMd: string }>(
      `spaces/${spaceId.value}/nodes/${current.value.id}/versions/${item.version}`,
    )
    versionContent.value = data.contentMd
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  } finally {
    versionLoading.value = false
  }
}

/** 版本差异（历史版本 → 当前草稿）。 */
const versionDiff = computed(() => diffLines(versionContent.value, draft.value))

/** 差异统计。 */
const versionDiffStat = computed(() => {
  const adds = versionDiff.value.filter((line) => line.kind === 'add').length
  const removes = versionDiff.value.filter((line) => line.kind === 'remove').length
  return { adds, removes }
})

/** 拉取当前文档评论。 */
async function loadComments() {
  if (!current.value) return
  commentsLoading.value = true
  try {
    comments.value = await api<CommentItem[]>(
      `spaces/${spaceId.value}/nodes/${current.value.id}/comments`,
    )
    const ids = [...new Set(comments.value.map((item) => item.authorId))]
    if (ids.length) {
      try {
        const users = await $fetch<Array<{ id: string; nickname: string }>>('/api/auth/users', {
          params: { ids: ids.join(',') },
          headers: auth.authHeaders(),
        })
        for (const user of users) commentUsers.value[user.id] = { nickname: user.nickname }
      } catch {
        // 昵称解析失败不影响评论展示
      }
    }
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  } finally {
    commentsLoading.value = false
  }
}

/** 打开/关闭评论抽屉。 */
async function toggleComments() {
  commentsOpen.value = !commentsOpen.value
  if (commentsOpen.value) await loadComments()
}

/** 以某个大纲标题为锚点写评论。 */
async function commentOnHeading(heading: string) {
  commentAnchor.value = heading
  replyTo.value = null
  commentsOpen.value = true
  await loadComments()
}

/** 发表评论或回复。 */
async function submitComment() {
  const body = commentBody.value.trim()
  if (!body || !current.value) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes/${current.value.id}/comments`, {
      method: 'POST',
      body: {
        body,
        parentId: replyTo.value?.id ?? null,
        anchor: commentAnchor.value ? { heading: commentAnchor.value } : null,
      },
    })
    commentBody.value = ''
    replyTo.value = null
    await loadComments()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 解决/取消解决。 */
async function resolveComment(comment: CommentItem) {
  try {
    const updated = await api<CommentItem>(
      `spaces/${spaceId.value}/nodes/${current.value!.id}/comments/${comment.id}`,
      { method: 'PATCH', body: { resolved: !comment.resolvedAt } },
    )
    Object.assign(comment, updated)
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 删除评论（作者或编辑者）。 */
async function removeComment(comment: CommentItem) {
  if (!window.confirm('删除这条评论？')) return
  try {
    await api(`spaces/${spaceId.value}/nodes/${current.value!.id}/comments/${comment.id}`, {
      method: 'DELETE',
    })
    await loadComments()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 评论按根/回复分组。 */
const commentThreads = computed(() => {
  const roots = comments.value.filter((item) => !item.parentId)
  return roots.map((root) => ({
    root,
    replies: comments.value.filter((item) => item.parentId === root.id),
  }))
})

/** 未解决的评论数。 */
const openCommentCount = computed(
  () => comments.value.filter((item) => !item.resolvedAt && !item.parentId).length,
)

/** 打开分享设置。 */
async function openShare() {
  if (!current.value) return
  shareCopied.value = false
  sharePassword.value = ''
  shareExpiry.value = ''
  visibility.value = current.value.visibility === 'private' ? 'private' : 'space'
  shareOpen.value = true
  await api<ShareInfo>(`spaces/${spaceId.value}/nodes/${current.value.id}/share`)
    .then((info) => {
      shareInfo.value = info
      shareExpiry.value = info.expiresAt
        ? new Date(info.expiresAt).toISOString().slice(0, 16)
        : ''
    })
    .catch(() => {
      shareInfo.value = { token: null, url: null, hasPassword: false, expiresAt: null, permission: null }
    })
}

/** 保存分享设置（生成或更新链接）。 */
async function saveShare() {
  if (!current.value || shareSaving.value) return
  shareSaving.value = true
  errorMessage.value = ''
  try {
    const body: Record<string, unknown> = { permission: 'read' }
    if (sharePassword.value.trim()) body.password = sharePassword.value.trim()
    if (shareExpiry.value) body.expiresAt = new Date(shareExpiry.value).toISOString()
    const info = await api<ShareInfo>(
      `spaces/${spaceId.value}/nodes/${current.value.id}/share`,
      { method: 'POST', body },
    )
    shareInfo.value = info
    sharePassword.value = ''
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  } finally {
    shareSaving.value = false
  }
}

/** 关闭分享。 */
async function revokeShare() {
  if (!current.value) return
  await api(`spaces/${spaceId.value}/nodes/${current.value.id}/share`, { method: 'DELETE' })
  shareInfo.value = { token: null, url: null, hasPassword: false, expiresAt: null, permission: null }
}

/** 复制分享链接。 */
async function copyShare() {
  if (!shareInfo.value?.url) return
  await navigator.clipboard.writeText(`${window.location.origin}${shareInfo.value.url}`)
  shareCopied.value = true
}

/** 切换文档可见性。 */
async function setVisibility(next: 'space' | 'private') {
  if (!current.value) return
  visibility.value = next
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/nodes/${current.value.id}`, {
      method: 'PATCH',
      body: { visibility: next },
    })
    current.value.visibility = next
    await loadTree()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 打开新建弹窗并加载模板。 */
async function openCreate(parent = '') {
  createParent.value = parent
  createTemplateId.value = ''
  createOpen.value = true
  if (spaceId.value) {
    templates.value = await api<TemplateItem[]>(`spaces/${spaceId.value}/templates`).catch(() => [])
  }
}

/** 新建页面/文件夹（可选择模板）。 */
async function createNode() {
  const title = createTitle.value.trim()
  if (!title || !spaceId.value) return
  errorMessage.value = ''
  try {
    if (createTemplateId.value && createKind.value === 'page') {
      await api(`spaces/${spaceId.value}/nodes/from-template`, {
        method: 'POST',
        body: {
          templateId: createTemplateId.value,
          title,
          parentId: createParent.value || null,
        },
      })
    } else {
      await api(`spaces/${spaceId.value}/nodes`, {
        method: 'POST',
        body: { kind: createKind.value, title, parentId: createParent.value || null },
      })
    }
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

/** 加载操作记录（审计接口返回 snake_case，这里统一成驼峰）。 */
async function loadChanges(nodeId: string) {
  try {
    const rows = await api<
      Array<{ id: string; action: string; createdAt?: string; created_at?: string }>
    >(`spaces/${spaceId.value}/nodes/${nodeId}/changes`)
    changes.value = rows.map((row) => ({
      id: row.id,
      action: row.action,
      createdAt: row.createdAt ?? row.created_at ?? '',
    }))
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

/** 正文字数（不含空白）。 */
const wordCount = computed(() => draft.value.replace(/\s/g, '').length)

/** 自动保存：编辑中且未保存时，停止输入 4 秒后保存。 */
let autosaveTimer: ReturnType<typeof setTimeout> | undefined
watch([draft, editing], () => {
  if (!editing.value || !dirty.value || saving.value) return
  clearTimeout(autosaveTimer)
  autosaveTimer = setTimeout(() => void save(), 4000)
})
onUnmounted(() => clearTimeout(autosaveTimer))

/** Ctrl/Cmd + S 保存。 */
function onKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
    event.preventDefault()
    void save()
  }
}

onMounted(() => {
  void loadRecent()
  window.addEventListener('keydown', onKeydown)
  window.addEventListener('click', closeTreeMenu)
  void loadSpaces()
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  window.removeEventListener('click', closeTreeMenu)
})
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
          class="shrink-0 rounded-[var(--radius-field)] p-1.5 text-neutral-400 transition hover:bg-[var(--surface-hover)] hover:text-neutral-600"
          title="空间设置"
          aria-label="空间设置"
          @click="openSpaceSettings"
        >
          <AppIcon name="settings" class="size-4" />
        </button>
      </div>

      <div class="px-2 py-2">
        <div class="relative">
          <AppIcon name="search" class="pointer-events-none absolute top-2 left-2 size-4 text-neutral-400" />
          <input
            v-model="keyword"
            placeholder="搜索标题与正文"
            class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] pr-2 pl-7 text-[12px] outline-none focus:border-primary-500 focus:bg-[var(--surface)] focus:ring-2 focus:ring-primary-500/20"
            @input="onKeywordInput"
          />
        </div>
      </div>

      <div
        v-if="!keyword.trim() && recentViews.length"
        class="border-b border-[var(--border)] px-2 py-2"
      >
        <p class="mb-1 px-1 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">
          最近浏览
        </p>
        <button
          v-for="item in recentViews.slice(0, 4)"
          :key="item.id"
          class="flex w-full items-center gap-1.5 rounded-[var(--radius-field)] px-1.5 py-1 text-left text-[12px] text-neutral-600 transition-colors hover:bg-[var(--surface-hover)] dark:text-neutral-300"
          @click="openRecent(item)"
        >
          <AppIcon name="clock" class="size-3.5 shrink-0 opacity-60" />
          <span class="min-w-0 flex-1 truncate">{{ item.title }}</span>
        </button>
      </div>

      <div v-if="keyword.trim()" class="flex-1 overflow-y-auto px-1.5 pb-2">
        <p v-if="searching" class="px-3 py-6 text-center text-[11px] text-neutral-400">搜索中…</p>
        <p
          v-else-if="searchResults.length === 0"
          class="px-3 py-6 text-center text-[11px] text-neutral-400"
        >
          没有找到包含「{{ keyword.trim() }}」的文档
        </p>
        <button
          v-for="hit in searchResults"
          :key="hit.id"
          class="mb-0.5 block w-full rounded-[var(--radius-field)] px-2 py-2 text-left transition-colors hover:bg-[var(--surface-hover)]"
          @click="openSearchHit(hit)"
        >
          <span class="flex items-center justify-between gap-2 text-[11px] text-neutral-400">
            <span class="truncate">{{ hit.spaceName }}</span>
            <span class="shrink-0">{{ new Date(hit.updatedAt).toLocaleDateString('zh-CN') }}</span>
          </span>
          <span class="mt-0.5 block truncate text-[12px] font-medium text-[var(--text-strong)]">
            {{ hit.title }}
          </span>
          <span class="mt-0.5 line-clamp-2 block text-[11px] text-neutral-500">{{ hit.snippet }}</span>
        </button>
      </div>

      <div v-else class="flex-1 overflow-y-auto px-1.5 pb-2">
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
          @contextmenu.prevent="openTreeMenu($event, row.node)"
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
            @click.stop="openCreate(row.node.id)"
          >
            <AppIcon name="plus" class="size-3" />
          </span>
        </button>
        <p v-if="flatTree.length === 0" class="py-8 text-center text-[11px] text-neutral-400">暂无文档</p>
      </div>

      <div class="border-t border-[var(--border)] p-2">
        <Button size="sm" variant="secondary" block @click="openCreate('')">
          <AppIcon name="plus" class="size-3.5" /> 新建
        </Button>
      </div>
    </aside>

    <!-- 树节点右键菜单 -->
    <div
      v-if="treeMenu.open && treeMenu.node"
      class="fixed z-50 w-40 overflow-hidden rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] py-1 text-[13px] shadow-[var(--shadow-pop)]"
      :style="{ left: `${treeMenu.x}px`, top: `${treeMenu.y}px` }"
      @click.stop
    >
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="startRename(treeMenu.node!)"
      >重命名</button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="startMove(treeMenu.node!)"
      >移动到…</button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="moveSibling(treeMenu.node!, -1)"
      >上移</button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
        @click="moveSibling(treeMenu.node!, 1)"
      >下移</button>
      <button
        class="block w-full px-3 py-1.5 text-left text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10"
        @click="deleteNode(treeMenu.node!)"
      >删除</button>
    </div>

    <!-- 空间设置 -->
    <AppModal :open="spaceSettingsOpen" title="空间设置" width="max-w-lg" @close="spaceSettingsOpen = false">
      <div class="space-y-4">
        <div>
          <h3 class="mb-1.5 text-[11px] font-medium text-neutral-500">成员（{{ spaceMembers.length }}）</h3>
          <div class="space-y-1">
            <div
              v-for="member in spaceMembers"
              :key="member.userId"
              class="flex items-center gap-2 rounded-[var(--radius-field)] border border-[var(--border)] px-2.5 py-1.5 text-[12px]"
            >
              <UserAvatar :name="spaceUsers[member.userId]?.nickname" :seed="member.userId" size="sm" />
              <span class="min-w-0 flex-1 truncate text-[var(--text-strong)]">
                {{ spaceUsers[member.userId]?.nickname ?? member.userId }}
                <span v-if="member.userId === auth.user?.id" class="ml-1 text-[10px] text-neutral-400">我</span>
              </span>
              <span
                v-if="member.role === 'owner'"
                class="rounded bg-primary-50 px-1 text-[10px] text-primary-700 dark:bg-primary-900/40 dark:text-primary-200"
              >所有者</span>
              <template v-else-if="mySpaceRole === 'owner' || (mySpaceRole === 'admin' && member.role !== 'admin')">
                <select
                  :value="member.role"
                  class="h-7 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] px-1 text-[11px] text-neutral-600"
                  @change="changeMemberRole(member, ($event.target as HTMLSelectElement).value)"
                >
                  <option v-if="mySpaceRole === 'owner'" value="admin">管理员</option>
                  <option value="editor">可编辑</option>
                  <option value="viewer">只读</option>
                </select>
                <button class="text-[11px] text-neutral-400 hover:text-danger-500" @click="removeSpaceMember(member)">
                  移除
                </button>
              </template>
              <template v-else>
                <span class="text-[10px] text-neutral-400">
                  {{ member.role === 'admin' ? '管理员' : member.role === 'editor' ? '可编辑' : '只读' }}
                </span>
              </template>
              <button
                v-if="mySpaceRole === 'owner' && member.role === 'editor'"
                class="text-[11px] text-neutral-400 hover:text-primary-600"
                @click="transferSpace(member)"
              >
                设为所有者
              </button>
            </div>
          </div>
        </div>

        <div v-if="mySpaceRole === 'owner' || mySpaceRole === 'admin'" class="rounded-[var(--radius-field)] border border-[var(--border)] p-3">
          <h3 class="mb-1.5 text-[11px] font-medium text-neutral-500">邀请成员</h3>
          <UserPicker v-model="inviteIds" multiple :exclude-ids="spaceMembers.map((row) => row.userId)" placeholder="搜索并选择成员" />
          <div class="mt-2 flex items-center gap-2">
            <select
              v-model="inviteRole"
              class="h-8 rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px]"
            >
              <option v-if="mySpaceRole === 'owner'" value="admin">管理员</option>
              <option value="editor">可编辑</option>
              <option value="viewer">只读</option>
            </select>
            <Button size="sm" :disabled="inviteIds.length === 0" @click="inviteMembers">邀请</Button>
          </div>
        </div>

        <p v-if="spaceError" class="text-[11px] text-danger-500">{{ spaceError }}</p>

        <div v-if="mySpaceRole === 'owner'" class="rounded-[var(--radius-field)] border border-danger-500/30 p-3">
          <p class="text-[12px] text-neutral-500">删除知识库会同时删除其中所有文档，不可恢复。</p>
          <Button variant="danger" size="sm" class="mt-2" @click="removeSpace">删除知识库</Button>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="spaceSettingsOpen = false">关闭</Button>
      </template>
    </AppModal>

    <!-- 分享 -->
    <AppModal :open="shareOpen" title="分享文档" @close="shareOpen = false">
      <div class="space-y-4">
        <label class="block">
          <span class="mb-1 block text-[11px] font-medium text-neutral-500">可见性</span>
          <select
            :value="visibility"
            class="h-9 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[13px] text-[var(--text-strong)] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
            @change="setVisibility(($event.target as HTMLSelectElement).value as 'space' | 'private')"
          >
            <option value="space">空间成员可见</option>
            <option value="private">仅编辑者可见</option>
          </select>
        </label>

        <div class="rounded-[var(--radius-field)] border border-[var(--border)] p-3">
          <p class="text-[12px] text-[var(--text-strong)]">分享链接</p>
          <p class="mt-0.5 text-[11px] text-neutral-400">
            任何拿到链接的人都能只读查看（可设置密码与有效期）。
          </p>
          <div v-if="shareInfo?.token" class="mt-2 flex items-center gap-2">
            <input
              readonly
              :value="shareInfo.url ?? ''"
              class="h-8 min-w-0 flex-1 rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] px-2 text-[11px] text-neutral-500"
            />
            <Button size="sm" variant="secondary" @click="copyShare">
              {{ shareCopied ? '已复制' : '复制' }}
            </Button>
          </div>
          <p v-else class="mt-2 text-[11px] text-neutral-400">尚未开启分享</p>
          <div class="mt-3 grid grid-cols-2 gap-2">
            <label class="block">
              <span class="mb-1 block text-[11px] text-neutral-500">访问密码（可选）</span>
              <input
                v-model="sharePassword"
                type="password"
                :placeholder="shareInfo?.hasPassword ? '留空则取消密码' : '不设置'"
                class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
              />
            </label>
            <label class="block">
              <span class="mb-1 block text-[11px] text-neutral-500">有效期（可选）</span>
              <input
                v-model="shareExpiry"
                type="datetime-local"
                class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[12px] text-[var(--text-strong)] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
              />
            </label>
          </div>
          <div class="mt-3 flex items-center gap-2">
            <Button size="sm" :loading="shareSaving" @click="saveShare">
              {{ shareInfo?.token ? '更新分享' : '生成链接' }}
            </Button>
            <Button v-if="shareInfo?.token" size="sm" variant="secondary" @click="revokeShare">关闭分享</Button>
          </div>
        </div>
      </div>
      <template #footer>
        <Button variant="secondary" @click="shareOpen = false">完成</Button>
      </template>
    </AppModal>

    <!-- 重命名 -->
    <AppModal :open="renameOpen" title="重命名" @close="renameOpen = false">
      <input
        v-model="renameTitle"
        class="h-9 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2.5 text-[13px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
        @keydown.enter.prevent="submitRename"
      />
      <template #footer>
        <Button variant="secondary" @click="renameOpen = false">取消</Button>
        <Button :disabled="!renameTitle.trim()" @click="submitRename">保存</Button>
      </template>
    </AppModal>

    <!-- 移动 -->
    <AppModal :open="moveOpen" title="移动到" @close="moveOpen = false">
      <label class="block text-[12px] text-neutral-500">
        目标位置（根目录或文件夹）
        <select
          v-model="moveParent"
          class="mt-1.5 h-9 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[13px] text-[var(--text-strong)] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
        >
          <option value="">根目录</option>
          <option
            v-for="folder in folders"
            :key="folder.id"
            :value="folder.id"
            :disabled="folder.id === moveTarget?.id"
          >
            {{ folder.label }}
          </option>
        </select>
      </label>
      <template #footer>
        <Button variant="secondary" @click="moveOpen = false">取消</Button>
        <Button @click="submitMove">移动</Button>
      </template>
    </AppModal>

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
              :class="following ? 'text-warning-700' : 'text-neutral-400'"
              :title="following ? '已关注，点击取消' : '关注文档更新'"
              aria-label="关注文档"
              @click="toggleFollow"
            >
              <AppIcon name="star" class="size-4" :class="following ? 'fill-current' : ''" />
            </button>
            <button
              class="relative rounded-[var(--radius-field)] p-1 transition hover:bg-[var(--surface-hover)]"
              :class="commentsOpen ? 'text-primary-600' : 'text-neutral-400'"
              aria-label="评论"
              @click="toggleComments"
            >
              <AppIcon name="chat" class="size-4" />
              <span
                v-if="openCommentCount > 0"
                class="absolute -top-0.5 -right-0.5 rounded-full bg-primary-600 px-1 text-[9px] leading-3 font-medium text-white"
              >{{ openCommentCount }}</span>
            </button>
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
<template v-else>
              <Button size="sm" variant="secondary" @click="openShare">分享</Button>
              <Button size="sm" @click="editing = true">编辑</Button>
              <div class="relative">
                <button
                  class="rounded-[var(--radius-field)] p-1 text-neutral-400 transition hover:bg-[var(--surface-hover)] hover:text-neutral-600"
                  aria-label="更多"
                  @click.stop="moreOpen = !moreOpen"
                >
                  <AppIcon name="menu" class="size-4" />
                </button>
                <div
                  v-if="moreOpen"
                  class="absolute right-0 z-40 mt-1 w-40 overflow-hidden rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface)] py-1 text-[13px] shadow-[var(--shadow-pop)]"
                  @click.stop
                >
                  <button
                    class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
                    @click="exportDoc('md')"
                  >导出 Markdown</button>
                  <button
                    class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
                    @click="exportDoc('html')"
                  >导出 HTML</button>
                  <button
                    class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
                    @click="pickImport"
                  >导入 Markdown</button>
                  <button
                    class="block w-full px-3 py-1.5 text-left hover:bg-[var(--surface-hover)]"
                    @click="saveAsTemplate"
                  >存为模板</button>
                </div>
              </div>
              <input
                ref="importInputEl"
                type="file"
                accept=".md,.markdown,.txt"
                class="hidden"
                @change="onImportPicked"
              />
            </template>
          </div>
        </div>

        <p v-if="errorMessage" class="px-6 pt-3 text-[12px] text-danger-500">{{ errorMessage }}</p>

        <div class="flex min-h-0 flex-1">
          <div class="min-w-0 flex-1 overflow-y-auto">
            <div class="mx-auto max-w-[760px] px-8 py-8">
              <h1 class="text-[26px] leading-tight font-semibold tracking-[-0.02em] text-[var(--text-strong)]">
                {{ current.title }}
              </h1>
              <p class="mt-1.5 mb-6 text-[11px] text-neutral-400">
                v{{ current.version }} · {{ wordCount }} 字
                <template v-if="current.viewCount"> · {{ current.viewCount }} 人浏览</template>
                · Ctrl/Cmd + S 保存
                <span v-if="editing" class="ml-1 text-neutral-300">（停止输入 4 秒自动保存）</span>
              </p>
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
            v-if="outlineOpen || historyOpen || commentsOpen"
            class="hidden w-[224px] shrink-0 overflow-y-auto border-l border-[var(--border)] px-3 py-4 lg:block"
          >
            <template v-if="outlineOpen">
              <h2 class="mb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">大纲</h2>
              <div
                v-for="(item, index) in outline"
                :key="`${index}-${item.text}`"
                class="group flex items-center gap-1 rounded-[var(--radius-field)] pr-1 hover:bg-[var(--surface-hover)]"
                :style="{ paddingLeft: `${(item.level - 1) * 10 + 6}px` }"
              >
                <button
                  class="min-w-0 flex-1 truncate py-1 text-left text-[12px] text-neutral-500 transition hover:text-[var(--text-strong)]"
                  @click="jumpToHeading(index)"
                >
                  {{ item.text }}
                </button>
                <button
                  class="hidden shrink-0 rounded p-0.5 text-neutral-400 hover:text-primary-600 group-hover:block"
                  title="就该标题评论"
                  @click="commentOnHeading(item.text)"
                >
                  <AppIcon name="chat" class="size-3" />
                </button>
              </div>
              <p v-if="outline.length === 0" class="text-[11px] text-neutral-400">暂无标题</p>
            </template>

            <template v-if="commentsOpen">
              <h2 class="mt-5 mb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase first:mt-0">
                评论（{{ openCommentCount }} 未解决）
              </h2>
              <p v-if="commentsLoading" class="py-3 text-center text-[11px] text-neutral-400">加载中…</p>
              <p
                v-else-if="comments.length === 0"
                class="py-3 text-[11px] text-neutral-400"
              >
                还没有评论
              </p>
              <div v-for="thread in commentThreads" :key="thread.root.id" class="mb-3">
                <div
                  class="rounded-[var(--radius-field)] border border-[var(--border)] p-2"
                  :class="thread.root.resolvedAt ? 'opacity-60' : ''"
                >
                  <p class="flex items-center justify-between gap-1 text-[10px] text-neutral-400">
                    <span class="truncate">
                      {{ commentUsers[thread.root.authorId]?.nickname ?? '成员' }}
                      <span v-if="thread.root.anchor?.heading" class="text-neutral-300">
                        · {{ thread.root.anchor.heading }}
                      </span>
                    </span>
                    <span v-if="thread.root.resolvedAt" class="shrink-0 text-success-700">已解决</span>
                  </p>
                  <p class="mt-1 break-words text-[12px] text-[var(--text-strong)]">
                    {{ thread.root.body }}
                  </p>
                  <div class="mt-1 flex items-center gap-2 text-[10px] text-neutral-400">
                    <button class="hover:text-primary-600" @click="replyTo = thread.root">
                      回复
                    </button>
                    <button class="hover:text-primary-600" @click="resolveComment(thread.root)">
                      {{ thread.root.resolvedAt ? '重新打开' : '解决' }}
                    </button>
                    <button class="hover:text-danger-500" @click="removeComment(thread.root)">
                      删除
                    </button>
                  </div>
                  <div
                    v-for="reply in thread.replies"
                    :key="reply.id"
                    class="mt-2 border-l border-[var(--border)] pl-2"
                  >
                    <p class="text-[10px] text-neutral-400">
                      {{ commentUsers[reply.authorId]?.nickname ?? '成员' }}
                    </p>
                    <p class="break-words text-[12px] text-neutral-600 dark:text-neutral-300">
                      {{ reply.body }}
                    </p>
                  </div>
                </div>
              </div>
              <div class="mt-2">
                <p v-if="replyTo" class="mb-1 text-[10px] text-neutral-400">
                  回复 {{ commentUsers[replyTo.authorId]?.nickname ?? '成员' }} ·
                  <button class="text-primary-600" @click="replyTo = null">取消</button>
                </p>
                <p v-else-if="commentAnchor" class="mb-1 text-[10px] text-neutral-400">
                  锚点：{{ commentAnchor }} ·
                  <button class="text-primary-600" @click="commentAnchor = ''">清除</button>
                </p>
                <textarea
                  v-model="commentBody"
                  rows="2"
                  placeholder="写评论，Ctrl/Cmd + Enter 发送"
                  class="w-full resize-none rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 py-1.5 text-[12px] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
                  @keydown.meta.enter.prevent="submitComment"
                  @keydown.ctrl.enter.prevent="submitComment"
                />
                <Button size="sm" class="mt-1.5" :disabled="!commentBody.trim()" @click="submitComment">
                  发送
                </Button>
              </div>
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
                class="flex items-center justify-between gap-2 rounded-[var(--radius-field)] px-1 py-1 text-[11px]"
                :class="selectedVersion === item.version ? 'bg-[var(--surface-hover)]' : ''"
              >
                <button class="min-w-0 flex-1 truncate text-left text-neutral-500 hover:text-[var(--text-strong)]" @click="openVersion(item)">
                  v{{ item.version }} · {{ new Date(item.createdAt).toLocaleDateString('zh-CN') }}
                </button>
                <button class="shrink-0 text-primary-600 hover:underline" @click="restore(item.version)">回滚</button>
              </div>

              <div v-if="selectedVersion !== null" class="mt-3">
                <p class="mb-1 flex items-center justify-between text-[10px] text-neutral-400">
                  <span>v{{ selectedVersion }} → 当前</span>
                  <span>
                    <span class="text-success-700">+{{ versionDiffStat.adds }}</span>
                    <span class="ml-1 text-danger-500">-{{ versionDiffStat.removes }}</span>
                  </span>
                </p>
                <p v-if="versionLoading" class="py-2 text-center text-[11px] text-neutral-400">加载中…</p>
                <pre
                  v-else
                  class="max-h-72 overflow-auto rounded-[var(--radius-field)] border border-[var(--border)] bg-[var(--surface-muted)] p-2 font-mono text-[11px] leading-5"
                ><template v-for="(line, index) in versionDiff" :key="index"><span
                    class="block break-all whitespace-pre-wrap"
                    :class="line.kind === 'add' ? 'text-success-700 bg-success-50 dark:bg-success-500/10' : line.kind === 'remove' ? 'text-danger-500 bg-danger-50 dark:bg-danger-500/10' : 'text-neutral-500'"
                  >{{ line.kind === 'add' ? '+' : line.kind === 'remove' ? '-' : ' ' }} {{ line.text }}</span></template></pre>
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
      <label v-if="createKind === 'page' && templates.length" class="mt-3 block">
        <span class="mb-1 block text-[11px] font-medium text-neutral-500">从模板创建（可选）</span>
        <select
          v-model="createTemplateId"
          class="h-8 w-full rounded-[var(--radius-field)] border border-[var(--border-strong)] bg-[var(--surface)] px-2 text-[13px] text-[var(--text-strong)] outline-none focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
        >
          <option value="">空白文档</option>
          <option v-for="template in templates" :key="template.id" :value="template.id">
            {{ template.name }}
          </option>
        </select>
      </label>
      <template #footer>
        <Button variant="secondary" @click="createOpen = false">取消</Button>
        <Button :disabled="!createTitle.trim()" @click="createNode">创建</Button>
      </template>
    </AppModal>
  </div>
</template>
