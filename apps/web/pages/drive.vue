<script setup lang="ts">
/** 网盘：空间/目录浏览、拖拽多文件上传、预览下载、分享（密码/有效期/二维码）、回收站。 */
import { Button, Input } from '@club-oa/ui'

interface SpaceItem {
  id: string
  name: string
  type: string
}
interface NodeItem {
  id: string
  parentId: string | null
  name: string
  kind: 'folder' | 'file'
  size: number
  mime: string | null
  createdAt: string
}

const auth = useAuthStore()

const spaces = ref<SpaceItem[]>([])
const spaceId = ref('')
const nodes = ref<NodeItem[]>([])
const trashed = ref<NodeItem[]>([])
const folderStack = ref<Array<{ id: string; name: string }>>([])
const showTrash = ref(false)
const viewMode = ref<'grid' | 'list'>('grid')
const keyword = ref('')
const errorMessage = ref('')
const uploading = ref(false)
const uploadTotal = ref(0)
const uploadDone = ref(0)
const dragging = ref(false)
const menu = reactive<{ open: boolean; x: number; y: number; node: NodeItem | null }>({
  open: false,
  x: 0,
  y: 0,
  node: null,
})
const changesOpen = ref(false)
const changes = ref<Array<{ id: string; action: string; createdAt: string }>>([])
const changesNode = ref<NodeItem | null>(null)

const createSpaceOpen = ref(false)
const newSpaceName = ref('')
const folderOpen = ref(false)
const newFolderName = ref('')

const shareOpen = ref(false)
const shareNode = ref<NodeItem | null>(null)
const sharePassword = ref('')
const shareExpires = ref('0')
const shareMaxDownloads = ref(0)
const shareUrl = ref('')
const shareId = ref('')
const shareQr = ref('')

const currentParentId = computed(() => folderStack.value.at(-1)?.id ?? '')
const currentSpace = computed(() => spaces.value.find((item) => item.id === spaceId.value) ?? null)
const filteredNodes = computed(() => {
  const key = keyword.value.trim().toLowerCase()
  if (!key) return nodes.value
  return nodes.value.filter((node) => node.name.toLowerCase().includes(key))
})

/** 带鉴权的 API 请求。 */
function api<T>(path: string, options: Record<string, unknown> = {}) {
  return $fetch<T>(`/api/drive/${path}`, {
    ...options,
    headers: { ...auth.authHeaders(), ...((options.headers as Record<string, string> | undefined) ?? {}) },
  })
}

/** 加载空间。 */
async function loadSpaces() {
  spaces.value = await api<SpaceItem[]>('spaces')
  if (spaces.value.length > 0 && !spaceId.value) spaceId.value = spaces.value[0].id
}

/** 加载当前目录 / 回收站。 */
async function loadNodes() {
  if (!spaceId.value) return
  if (showTrash.value) {
    trashed.value = await api<NodeItem[]>(`spaces/${spaceId.value}/nodes?trashed=true`)
    return
  }
  const query = currentParentId.value ? `?parentId=${currentParentId.value}` : ''
  nodes.value = await api<NodeItem[]>(`spaces/${spaceId.value}/nodes${query}`)
}

/** 切换空间。 */
async function switchSpace() {
  folderStack.value = []
  showTrash.value = false
  await loadNodes()
}

/** 创建空间。 */
async function createSpace() {
  const name = newSpaceName.value.trim()
  if (!name) return
  const created = await api<SpaceItem>('spaces', { method: 'POST', body: { name, type: 'team' } })
  newSpaceName.value = ''
  createSpaceOpen.value = false
  await loadSpaces()
  spaceId.value = created.id ?? spaceId.value
  await switchSpace()
}

/** 进入目录。 */
async function openFolder(node: NodeItem) {
  folderStack.value.push({ id: node.id, name: node.name })
  await loadNodes()
}

/** 返回上级。 */
async function goUp(index: number) {
  folderStack.value = folderStack.value.slice(0, index)
  await loadNodes()
}

/** 新建文件夹。 */
async function createFolder() {
  const name = newFolderName.value.trim()
  if (!name) return
  errorMessage.value = ''
  try {
    await api(`spaces/${spaceId.value}/folders`, {
      method: 'POST',
      body: { name, parentId: currentParentId.value || null },
    })
    newFolderName.value = ''
    folderOpen.value = false
    await loadNodes()
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 上传单个文件（预签名直传优先，回退服务端中转）。 */
async function uploadFile(file: File) {
  const mime = file.type || 'application/octet-stream'
  let presign: { storageKey: string; uploadUrl: string } | null = null
  try {
    presign = await api<{ storageKey: string; uploadUrl: string }>(
      `spaces/${spaceId.value}/files/presign`,
      { method: 'POST', body: { name: file.name, mime, size: file.size, parentId: currentParentId.value || undefined } },
    )
  } catch (error) {
    const code = (error as { data?: { code?: string } }).data?.code
    if (code !== 'DRIVE_PRESIGN_UNSUPPORTED') throw error
  }
  if (presign) {
    const put = await fetch(presign.uploadUrl, { method: 'PUT', body: file, headers: { 'content-type': mime } })
    if (!put.ok) throw new Error(`直传失败（${put.status}）`)
    await api(`spaces/${spaceId.value}/files/complete`, {
      method: 'POST',
      body: { storageKey: presign.storageKey, name: file.name, mime, size: file.size, parentId: currentParentId.value || undefined },
    })
  } else {
    const query = new URLSearchParams({ name: file.name, mime })
    if (currentParentId.value) query.set('parentId', currentParentId.value)
    await api(`spaces/${spaceId.value}/files?${query.toString()}`, {
      method: 'POST',
      body: file,
      headers: { 'content-type': mime },
    })
  }
}

/** 批量上传。 */
async function uploadFiles(files: File[]) {
  if (!files.length || !spaceId.value) return
  uploading.value = true
  uploadTotal.value = files.length
  uploadDone.value = 0
  errorMessage.value = ''
  try {
    for (const file of files) {
      await uploadFile(file)
      uploadDone.value += 1
    }
    await loadNodes()
  } catch (error) {
    errorMessage.value = `上传失败：${apiErrorMessage(error)}`
  } finally {
    uploading.value = false
  }
}

/** 选择文件上传。 */
function onPickFiles(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  void uploadFiles(files)
}

/** 拖拽上传。 */
function onDrop(event: DragEvent) {
  dragging.value = false
  const files = Array.from(event.dataTransfer?.files ?? [])
  void uploadFiles(files)
}

/** 下载。 */
async function download(node: NodeItem) {
  const response = await fetch(`/api/drive/download?spaceId=${spaceId.value}&nodeId=${node.id}`, {
    headers: auth.authHeaders(),
  })
  if (!response.ok) {
    errorMessage.value = '下载失败'
    return
  }
  const blob = await response.blob()
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = node.name
  link.click()
  URL.revokeObjectURL(url)
}

/** 删除（进回收站）/ 恢复。 */
async function removeNode(node: NodeItem) {
  await api(`spaces/${spaceId.value}/nodes/${node.id}`, { method: 'DELETE' })
  await loadNodes()
}
async function restoreNode(node: NodeItem) {
  await api(`spaces/${spaceId.value}/nodes/${node.id}/restore`, { method: 'POST' })
  await loadNodes()
}

/** 打开文件操作记录。 */
async function openChanges(node: NodeItem) {
  changesNode.value = node
  try {
    changes.value = await api(`spaces/${spaceId.value}/nodes/${node.id}/changes`)
  } catch {
    changes.value = []
  }
  changesOpen.value = true
}

/** 撤销删除。 */
async function undoChange(change: { id: string }) {
  await api(`changes/${change.id}/undo`, { method: 'POST' })
  if (changesNode.value) {
    changes.value = await api(`spaces/${spaceId.value}/nodes/${changesNode.value.id}/changes`).catch(() => [])
  }
  await loadNodes()
}

/** 打开右键菜单。 */
function openMenu(event: MouseEvent, node: NodeItem) {
  menu.open = true
  menu.x = event.clientX
  menu.y = event.clientY
  menu.node = node
}

/** 关闭右键菜单。 */
function closeMenu() {
  menu.open = false
  menu.node = null
}

/** 菜单动作后关闭。 */
function runMenu(action: () => void) {
  action()
  closeMenu()
}

/** 打开节点（目录进入，文件预览/下载）。 */
function openNode(node: NodeItem) {
  if (node.kind === 'folder') return void openFolder(node)
  if (canPreview(node)) return preview(node)
  return void download(node)
}

/** 打开分享弹窗。 */
async function openShare(node: NodeItem) {
  shareNode.value = node
  sharePassword.value = ''
  shareExpires.value = '0'
  shareMaxDownloads.value = 0
  shareUrl.value = ''
  shareId.value = ''
  shareQr.value = ''
  shareOpen.value = true
}

/** 生成分享链接。 */
async function createShare() {
  if (!shareNode.value) return
  errorMessage.value = ''
  try {
    const body: Record<string, unknown> = { maxDownloads: shareMaxDownloads.value }
    const expires = Number(shareExpires.value)
    if (expires > 0) body.expiresInSeconds = expires
    if (sharePassword.value.trim()) body.password = sharePassword.value.trim()
    const result = await api<{ id: string; token: string; url: string }>(
      `spaces/${spaceId.value}/nodes/${shareNode.value.id}/shares`,
      { method: 'POST', body },
    )
    shareId.value = result.id
    shareUrl.value = `${window.location.origin}${result.url}`
    const { default: QRCode } = await import('qrcode')
    shareQr.value = await QRCode.toDataURL(shareUrl.value, { width: 320, margin: 1 })
  } catch (error) {
    errorMessage.value = apiErrorMessage(error)
  }
}

/** 吊销分享。 */
async function revokeShare() {
  if (!shareId.value) return
  await api(`spaces/${spaceId.value}/shares/${shareId.value}`, { method: 'DELETE' })
  shareOpen.value = false
}

/** 复制分享链接。 */
async function copyShare() {
  if (!shareUrl.value) return
  await navigator.clipboard.writeText(shareUrl.value)
}

/** Office 预览。 */
function preview(node: NodeItem) {
  navigateTo(`/office/${node.id}?spaceId=${spaceId.value}`)
}
function canPreview(node: NodeItem) {
  if (node.kind !== 'file') return false
  const ext = node.name.split('.').pop()?.toLowerCase() ?? ''
  return ['doc', 'docx', 'odt', 'rtf', 'txt', 'xls', 'xlsx', 'ods', 'csv', 'ppt', 'pptx', 'odp', 'pdf'].includes(ext)
}

/** 文件类型样式（图标 + 配色）。 */
function fileStyle(node: NodeItem) {
  if (node.kind === 'folder') {
    return { icon: 'folder', class: 'bg-amber-50 text-amber-600 dark:bg-amber-500/10' }
  }
  const ext = node.name.split('.').pop()?.toLowerCase() ?? ''
  if (['doc', 'docx', 'odt', 'rtf'].includes(ext)) return { icon: 'doc', class: 'bg-info-50 text-info-700 dark:bg-info-500/10' }
  if (['xls', 'xlsx', 'ods', 'csv'].includes(ext)) return { icon: 'board', class: 'bg-success-50 text-success-700 dark:bg-success-500/10' }
  if (['ppt', 'pptx', 'odp'].includes(ext)) return { icon: 'doc', class: 'bg-warning-50 text-warning-700 dark:bg-warning-500/10' }
  if (ext === 'pdf') return { icon: 'doc', class: 'bg-danger-50 text-danger-700 dark:bg-danger-500/10' }
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'heic'].includes(ext)) return { icon: 'folder', class: 'bg-violet-50 text-violet-600 dark:bg-violet-500/10' }
  if (['zip', 'rar', '7z', 'tar', 'gz'].includes(ext)) return { icon: 'folder', class: 'bg-neutral-100 text-neutral-600 dark:bg-neutral-800' }
  return { icon: 'doc', class: 'bg-neutral-100 text-neutral-500 dark:bg-neutral-800' }
}

/** 大小展示。 */
function formatSize(size: number) {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  if (size < 1024 * 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`
  return `${(size / 1024 / 1024 / 1024).toFixed(2)} GB`
}

onMounted(async () => {
  window.addEventListener('click', closeMenu)
  await loadSpaces()
  await loadNodes()
})
onUnmounted(() => window.removeEventListener('click', closeMenu))
</script>

<template>
  <div
    class="flex h-[calc(100vh-7.5rem)] gap-4"
    @dragover.prevent="dragging = true"
    @dragleave="dragging = false"
    @drop.prevent="onDrop"
  >
    <!-- 侧栏 -->
    <aside class="hidden w-56 shrink-0 flex-col md:flex">
      <div class="mb-3 flex items-center justify-between px-1">
        <h1 class="text-base font-semibold">网盘</h1>
        <button
          class="rounded p-1 text-neutral-400 hover:bg-neutral-100 dark:hover:bg-neutral-800"
          aria-label="新建空间"
          @click="createSpaceOpen = true"
        >
          <AppIcon name="plus" class="size-4" />
        </button>
      </div>
      <nav class="space-y-0.5">
        <button
          v-for="space in spaces"
          :key="space.id"
          class="flex w-full items-center gap-2 rounded-[var(--radius-field)] px-2.5 py-2 text-left text-sm transition"
          :class="
            space.id === spaceId && !showTrash
              ? 'bg-primary-50 font-medium text-primary-700 dark:bg-primary-900/40 dark:text-primary-200'
              : 'text-neutral-600 hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800'
          "
          @click="spaceId = space.id; switchSpace()"
        >
          <AppIcon name="folder" class="size-4.5 shrink-0 opacity-80" />
          <span class="truncate">{{ space.name }}</span>
        </button>
      </nav>
      <div class="mt-3 border-t border-neutral-200 pt-3 dark:border-neutral-800">
        <button
          class="flex w-full items-center gap-2 rounded-[var(--radius-field)] px-2.5 py-2 text-sm transition"
          :class="
            showTrash
              ? 'bg-primary-50 font-medium text-primary-700 dark:bg-primary-900/40'
              : 'text-neutral-500 hover:bg-neutral-100 dark:hover:bg-neutral-800'
          "
          @click="showTrash = !showTrash; loadNodes()"
        >
          <AppIcon name="menu" class="size-4.5" />
          回收站
          <span v-if="trashed.length && showTrash" class="text-xs text-neutral-400">{{ trashed.length }}</span>
        </button>
      </div>
      <p class="mt-auto px-1 text-[11px] leading-5 text-neutral-400">
        支持拖拽文件到右侧任意位置上传
      </p>
    </aside>

    <!-- 主区域 -->
    <div class="flex min-w-0 flex-1 flex-col rounded-[var(--radius-card)] border border-neutral-200 bg-white shadow-[var(--shadow-card)] dark:border-neutral-800 dark:bg-neutral-900">
      <!-- 工具栏 -->
      <div class="border-b border-neutral-200 px-4 py-3 dark:border-neutral-800">
        <div class="flex flex-wrap items-center gap-2">
          <nav class="flex min-w-0 flex-1 items-center gap-1 text-sm">
            <button class="shrink-0 font-medium text-primary-600 hover:underline" @click="goUp(0)">
              {{ showTrash ? '回收站' : (currentSpace?.name ?? '空间') }}
            </button>
            <template v-if="!showTrash">
              <template v-for="(folder, index) in folderStack" :key="folder.id">
                <span class="text-neutral-300">/</span>
                <button class="max-w-40 truncate text-neutral-500 hover:text-primary-600" @click="goUp(index + 1)">
                  {{ folder.name }}
                </button>
              </template>
            </template>
          </nav>
          <div class="flex items-center gap-1.5">
            <div class="relative hidden sm:block">
              <AppIcon name="search" class="absolute top-2.5 left-2.5 size-4 text-neutral-400" />
              <input
                v-model="keyword"
                placeholder="搜索当前目录"
                class="h-9 w-44 rounded-[var(--radius-field)] border border-neutral-300 bg-transparent pr-2 pl-8 text-sm outline-none focus:border-primary-500 dark:border-neutral-700"
              />
            </div>
            <button
              class="rounded-[var(--radius-field)] p-2 text-neutral-500 transition hover:bg-neutral-100 dark:hover:bg-neutral-800"
              :aria-label="viewMode === 'grid' ? '列表视图' : '网格视图'"
              @click="viewMode = viewMode === 'grid' ? 'list' : 'grid'"
            >
              <AppIcon name="board" class="size-4.5" />
            </button>
          </div>
        </div>
        <div v-if="!showTrash" class="mt-2 flex flex-wrap items-center gap-2">
          <label
            class="inline-flex cursor-pointer items-center gap-1.5 rounded-[var(--radius-field)] bg-primary-600 px-3 py-1.5 text-sm font-medium text-white transition hover:bg-primary-700"
          >
            <AppIcon name="plus" class="size-4" />
            {{ uploading ? `上传中 ${uploadDone}/${uploadTotal}` : '上传文件' }}
            <input type="file" class="hidden" multiple :disabled="uploading" @change="onPickFiles" />
          </label>
          <Button variant="secondary" size="sm" @click="folderOpen = true">
            <AppIcon name="folder" class="size-4" /> 新建文件夹
          </Button>
        </div>
      </div>

      <p v-if="errorMessage" class="px-4 pt-3 text-sm text-danger-500">{{ errorMessage }}</p>

      <!-- 内容 -->
      <div class="flex-1 overflow-y-auto px-4 py-4">
        <template v-if="showTrash">
          <div v-if="trashed.length" class="space-y-2">
            <div
              v-for="node in trashed"
              :key="node.id"
              class="flex items-center justify-between rounded-[var(--radius-field)] border border-neutral-200 px-3 py-2 text-sm dark:border-neutral-800"
            >
              <span class="truncate text-neutral-500">{{ node.name }}</span>
              <Button variant="ghost" size="sm" @click="restoreNode(node)">恢复</Button>
            </div>
          </div>
          <p v-else class="py-16 text-center text-sm text-neutral-400">回收站为空</p>
        </template>

        <template v-else>
          <div v-if="viewMode === 'grid'" class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
            <div
              v-for="node in filteredNodes"
              :key="node.id"
              class="group cursor-pointer rounded-[var(--radius-card)] border border-neutral-200 p-3 transition hover:-translate-y-0.5 hover:border-primary-300 hover:shadow-[var(--shadow-card)] dark:border-neutral-800 dark:hover:border-primary-700"
              @contextmenu.prevent="openMenu($event, node)"
              @dblclick="node.kind === 'folder' ? openFolder(node) : canPreview(node) ? preview(node) : download(node)"
            >
              <div class="flex items-start justify-between">
                <span class="flex size-10 items-center justify-center rounded-xl" :class="fileStyle(node).class">
                  <AppIcon :name="fileStyle(node).icon" class="size-5" />
                </span>
                <div class="flex gap-0.5 opacity-0 transition group-hover:opacity-100">
                  <button
                    v-if="canPreview(node)"
                    class="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-primary-600 dark:hover:bg-neutral-800"
                    aria-label="预览"
                    @click.stop="preview(node)"
                  >
                    <AppIcon name="search" class="size-4" />
                  </button>
                  <button
                    v-if="node.kind === 'file'"
                    class="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-primary-600 dark:hover:bg-neutral-800"
                    aria-label="下载"
                    @click.stop="download(node)"
                  >
                    <AppIcon name="logout" class="size-4" />
                  </button>
                  <button
                    class="rounded p-1 text-neutral-400 hover:bg-neutral-100 hover:text-primary-600 dark:hover:bg-neutral-800"
                    aria-label="分享"
                    @click.stop="openShare(node)"
                  >
                    <AppIcon name="users" class="size-4" />
                  </button>
                  <button
                    class="rounded p-1 text-neutral-400 hover:bg-danger-50 hover:text-danger-500 dark:hover:bg-danger-500/10"
                    aria-label="删除"
                    @click.stop="removeNode(node)"
                  >
                    ✕
                  </button>
                </div>
              </div>
              <button
                class="mt-3 block w-full truncate text-left text-sm font-medium"
                @click="node.kind === 'folder' ? openFolder(node) : canPreview(node) ? preview(node) : download(node)"
              >
                {{ node.name }}
              </button>
              <p class="mt-0.5 text-xs text-neutral-400">
                {{ node.kind === 'folder' ? '文件夹' : formatSize(node.size) }}
              </p>
            </div>
          </div>

          <div v-else class="divide-y divide-neutral-100 dark:divide-neutral-800">
            <div
              v-for="node in filteredNodes"
              :key="node.id"
              class="group flex items-center gap-3 py-2 text-sm"
              @contextmenu.prevent="openMenu($event, node)"
            >
              <span class="flex size-8 shrink-0 items-center justify-center rounded-lg" :class="fileStyle(node).class">
                <AppIcon :name="fileStyle(node).icon" class="size-4" />
              </span>
              <button
                class="min-w-0 flex-1 truncate text-left"
                @click="node.kind === 'folder' ? openFolder(node) : canPreview(node) ? preview(node) : download(node)"
              >
                {{ node.name }}
              </button>
              <span v-if="node.kind === 'file'" class="w-20 shrink-0 text-right text-xs text-neutral-400">
                {{ formatSize(node.size) }}
              </span>
              <div class="flex shrink-0 gap-0.5 opacity-0 transition group-hover:opacity-100">
                <Button v-if="canPreview(node)" variant="ghost" size="sm" @click="preview(node)">预览</Button>
                <Button v-if="node.kind === 'file'" variant="ghost" size="sm" @click="download(node)">下载</Button>
                <Button variant="ghost" size="sm" @click="openShare(node)">分享</Button>
                <Button variant="ghost" size="sm" @click="removeNode(node)">删除</Button>
              </div>
            </div>
          </div>

          <p
            v-if="filteredNodes.length === 0"
            class="py-16 text-center text-sm text-neutral-400"
          >
            {{ keyword ? '没有匹配的文件' : '空目录，拖拽文件到这里上传' }}
          </p>
        </template>
      </div>

      <div
        v-if="dragging"
        class="pointer-events-none absolute inset-2 flex items-center justify-center rounded-[var(--radius-card)] border-2 border-dashed border-primary-400 bg-primary-50/80 text-sm font-medium text-primary-700 dark:bg-primary-900/40"
      >
        松开鼠标上传到当前目录
      </div>
    </div>

    <div
      v-if="menu.open && menu.node"
      class="fixed z-50 w-40 overflow-hidden rounded-[var(--radius-field)] border border-neutral-200 bg-white py-1 text-sm shadow-[var(--shadow-pop)] dark:border-neutral-700 dark:bg-neutral-900"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
      @click.stop
    >
      <button
        v-if="menu.node.kind === 'folder' || canPreview(menu.node)"
        class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800"
        @click="runMenu(() => openNode(menu.node!))"
      >
        {{ menu.node.kind === 'folder' ? '打开' : '预览' }}
      </button>
      <button
        v-if="menu.node.kind === 'file'"
        class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800"
        @click="runMenu(() => download(menu.node!))"
      >
        下载
      </button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800"
        @click="runMenu(() => openShare(menu.node!))"
      >
        分享
      </button>
      <button
        class="block w-full px-3 py-1.5 text-left hover:bg-neutral-100 dark:hover:bg-neutral-800"
        @click="runMenu(() => openChanges(menu.node!))"
      >
        操作记录
      </button>
      <div class="my-1 border-t border-neutral-100 dark:border-neutral-800" />
      <button
        class="block w-full px-3 py-1.5 text-left text-danger-500 hover:bg-danger-50 dark:hover:bg-danger-500/10"
        @click="runMenu(() => removeNode(menu.node!))"
      >
        删除
      </button>
    </div>

    <AppModal :open="changesOpen" title="操作记录" @close="changesOpen = false">
      <p class="mb-2 truncate text-sm text-neutral-500">{{ changesNode?.name }}</p>
      <div v-if="changes.length" class="space-y-1.5">
        <div
          v-for="change in changes"
          :key="change.id"
          class="flex items-center justify-between rounded-[var(--radius-field)] border border-neutral-200 px-2.5 py-1.5 text-sm dark:border-neutral-700"
        >
          <span class="text-neutral-500">
            {{ change.action === 'delete' ? '删除' : change.action === 'undo' ? '撤销' : change.action }}
            · {{ new Date(change.createdAt).toLocaleString('zh-CN') }}
          </span>
          <button class="text-primary-600 hover:underline" @click="undoChange(change)">撤销</button>
        </div>
      </div>
      <p v-else class="text-sm text-neutral-400">暂无操作记录</p>
    </AppModal>

    <!-- 新建空间 -->
    <AppModal :open="createSpaceOpen" title="新建空间" @close="createSpaceOpen = false">
      <Input v-model="newSpaceName" placeholder="空间名称" @keyup.enter="createSpace" />
      <template #footer>
        <Button variant="secondary" @click="createSpaceOpen = false">取消</Button>
        <Button :disabled="!newSpaceName.trim()" @click="createSpace">创建</Button>
      </template>
    </AppModal>

    <!-- 新建文件夹 -->
    <AppModal :open="folderOpen" title="新建文件夹" @close="folderOpen = false">
      <Input v-model="newFolderName" placeholder="文件夹名称" @keyup.enter="createFolder" />
      <template #footer>
        <Button variant="secondary" @click="folderOpen = false">取消</Button>
        <Button :disabled="!newFolderName.trim()" @click="createFolder">创建</Button>
      </template>
    </AppModal>

    <!-- 分享 -->
    <AppModal :open="shareOpen" title="分享文件" @close="shareOpen = false">
      <p class="mb-3 truncate text-sm text-neutral-500">「{{ shareNode?.name }}」</p>
      <template v-if="!shareUrl">
        <div class="space-y-3">
          <label class="block text-sm">
            <span class="mb-1 block text-xs text-neutral-500">访问密码（可选）</span>
            <Input v-model="sharePassword" type="password" placeholder="4~64 位，留空则无密码" />
          </label>
          <div class="grid grid-cols-2 gap-3">
            <label class="block text-sm">
              <span class="mb-1 block text-xs text-neutral-500">有效期</span>
              <select
                v-model="shareExpires"
                class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700"
              >
                <option value="0">永久</option>
                <option value="86400">1 天</option>
                <option value="604800">7 天</option>
                <option value="2592000">30 天</option>
              </select>
            </label>
            <label class="block text-sm">
              <span class="mb-1 block text-xs text-neutral-500">下载次数上限</span>
              <input
                v-model.number="shareMaxDownloads"
                type="number"
                min="0"
                class="w-full rounded-[var(--radius-field)] border border-neutral-300 bg-transparent px-3 py-2 text-sm dark:border-neutral-700"
                placeholder="0 = 不限"
              />
            </label>
          </div>
          <p class="text-xs text-neutral-400">分享为只读，可随时吊销。</p>
        </div>
      </template>
      <template v-else>
        <div class="space-y-3">
          <div class="flex items-center gap-2">
            <input
              readonly
              :value="shareUrl"
              class="min-w-0 flex-1 rounded-[var(--radius-field)] border border-neutral-300 bg-neutral-50 px-2 py-1.5 text-xs dark:border-neutral-700 dark:bg-neutral-800"
              @focus="($event.target as HTMLInputElement).select()"
            />
            <Button size="sm" variant="secondary" @click="copyShare">复制</Button>
            <Button size="sm" variant="danger" @click="revokeShare">吊销</Button>
          </div>
          <img
            v-if="shareQr"
            :src="shareQr"
            alt="分享二维码"
            class="mx-auto size-56 rounded-lg border border-neutral-200 bg-white p-2 dark:border-neutral-700"
          />
          <p class="text-center text-xs text-neutral-400">扫码或复制链接即可访问</p>
        </div>
      </template>
      <template #footer>
        <Button variant="secondary" @click="shareOpen = false">关闭</Button>
        <Button v-if="!shareUrl" @click="createShare">生成链接</Button>
      </template>
    </AppModal>
  </div>
</template>
