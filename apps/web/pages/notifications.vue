<script setup lang="ts">
/** 通知中心：列表、未读数、已读操作与加载更多。 */
import { Button, Card } from '@club-oa/ui'

const store = useNotificationsStore()
const unreadOnly = ref(false)

/** 首次加载：未读数 + 列表。 */
onMounted(async () => {
  await Promise.all([store.fetchUnreadCount(), store.fetchList()])
})

/** 切换"仅未读"过滤。 */
async function toggleUnreadOnly() {
  unreadOnly.value = !unreadOnly.value
  await store.fetchList({ unreadOnly: unreadOnly.value })
}
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <h1 class="text-xl font-semibold">
        通知中心
        <span v-if="store.hasUnread" class="ml-2 rounded-full bg-danger-500 px-2 py-0.5 text-xs text-white">
          {{ store.unreadCount }}
        </span>
      </h1>
      <div class="flex gap-2">
        <Button variant="secondary" size="sm" @click="toggleUnreadOnly">
          {{ unreadOnly ? '显示全部' : '仅看未读' }}
        </Button>
        <Button size="sm" :disabled="!store.hasUnread" @click="store.markAllRead()">全部已读</Button>
      </div>
    </div>

    <Card v-if="store.items.length === 0" class="text-center text-sm text-neutral-500">
      暂无通知
    </Card>

    <Card v-for="item in store.items" :key="item.id" class="space-y-1" :class="item.read ? 'opacity-60' : ''">
      <div class="flex items-start justify-between gap-3">
        <div>
          <NuxtLink
            v-if="item.url"
            :to="item.url"
            class="font-medium hover:text-primary-600"
            @click="store.markRead(item.id)"
          >
            {{ item.title }}
          </NuxtLink>
          <span v-else class="font-medium">{{ item.title }}</span>
          <p class="mt-1 text-sm text-neutral-500">{{ item.body }}</p>
        </div>
        <Button v-if="!item.read" variant="ghost" size="sm" @click="store.markRead(item.id)">标记已读</Button>
      </div>
    </Card>

    <div v-if="store.nextCursor" class="text-center">
      <Button variant="secondary" :loading="store.loading" @click="store.fetchList({ unreadOnly, append: true })">
        加载更多
      </Button>
    </div>
  </div>
</template>
