<script setup lang="ts">
/** OnlyOffice 预览页：向 BFF 取配置，动态加载 DocsAPI 并以只读模式嵌入。 */
import { Button, Card } from '@club-oa/ui'

const route = useRoute()
const auth = useAuthStore()
const errorMessage = ref('')
const containerId = 'onlyoffice-editor'

/** 动态加载 OnlyOffice API 脚本（同一地址只加载一次）。 */
function loadScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    if (document.querySelector(`script[src="${src}"]`)) return resolve()
    const script = document.createElement('script')
    script.src = src
    script.onload = () => resolve()
    script.onerror = () => reject(new Error('DocsAPI 加载失败'))
    document.head.appendChild(script)
  })
}

onMounted(async () => {
  const spaceId = typeof route.query.spaceId === 'string' ? route.query.spaceId : ''
  if (!spaceId) {
    errorMessage.value = '缺少 spaceId 参数'
    return
  }
  try {
    const config = await $fetch<{
      documentServerUrl: string
      config: Record<string, unknown>
      token: string
    }>('/api/office/config', {
      query: { spaceId, nodeId: route.params.nodeId },
      headers: auth.authHeaders(),
    })
    await loadScript(`${config.documentServerUrl}/web-apps/apps/api/documents/api.js`)
    const docsApi = (window as unknown as { DocsAPI?: { DocEditor: new (id: string, config: unknown) => unknown } }).DocsAPI
    if (!docsApi) throw new Error('DocsAPI 未就绪')
    new docsApi.DocEditor(containerId, { ...config.config, token: config.token })
  } catch (error) {
    const code = (error as { data?: { code?: string } }).data?.code
    errorMessage.value = code === 'ONLYOFFICE_NOT_CONFIGURED' ? '管理员尚未配置 OnlyOffice' : '预览加载失败，请稍后重试'
  }
})
</script>

<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <h1 class="text-xl font-semibold">Office 预览</h1>
      <Button variant="ghost" size="sm" @click="$router.back()">返回</Button>
    </div>
    <Card v-if="errorMessage" class="text-sm text-danger-500">{{ errorMessage }}</Card>
    <div v-show="!errorMessage" :id="containerId" class="h-[calc(100vh-11rem)] w-full overflow-hidden rounded-[var(--radius-card)] border border-neutral-200" />
  </div>
</template>
