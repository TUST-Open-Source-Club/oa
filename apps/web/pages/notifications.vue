<script setup lang="ts">
/** 通知中心：列表、未读数、已读操作与加载更多。 */
import { Button } from '@club-oa/ui'

const store = useNotificationsStore()
const unreadOnly = ref(false)

/** 首次加载：未读数 + 列表。 */
onMounted(async () => {
  await Promise.all([store.fetchUnreadCount(), store.fetchList()])
})

/** 切换「仅未读」过滤。 */
async function toggleUnreadOnly() {
  unreadOnly.value = !unreadOnly.value
  await store.fetchList({ unreadOnly: unreadOnly.value })
}
</script>

<template>
  <div class="mx-auto w-full max-w-[860px]">
    <header class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <p class="text-[12px] text-neutral-500">任务、活动与系统消息</p>
        <span
          v-if="store.hasUnread"
          class="rounded-full bg-danger-50 px-1.5 text-[11px] font-medium text-danger-700"
        >
          {{ store.unreadCount }} 未读
        </span>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="secondary" size="sm" @click="toggleUnreadOnly">
          {{ unreadOnly ? '显示全部' : '仅看未读' }}
        </Button>
        <Button size="sm" :disabled="!store.hasUnread" @click="store.markAllRead()">全部已读</Button>
      </div>
    </header>

    <div
      class="mt-4 overflow-hidden rounded-[var(--radius-card)] border border-[var(--border)] bg-[var(--surface)] shadow-[var(--shadow-card)]"
    >
      <p v-if="store.items.length === 0" class="px-4 py-12 text-center text-[12px] text-neutral-400">
        暂无通知
      </p>

      <article
        v-for="(item, index) in store.items"
        :key="item.id"
        class="flex items-start gap-3 px-4 py-3 transition-colors hover:bg-[var(--surface-hover)]"
        :class="[index > 0 ? 'border-t border-[var(--border)]' : '', item.read ? 'opacity-60' : '']"
      >
        <span
          class="mt-1.5 size-1.5 shrink-0 rounded-full"
          :class="item.read ? 'bg-neutral-300' : 'bg-primary-500'"
        />
        <div class="min-w-0 flex-1">
          <div class="flex items-start justify-between gap-3">
            <NuxtLink
              v-if="item.url"
              :to="item.url"
              class="min-w-0 truncate text-[13px] font-medium text-[var(--text-strong)] hover:text-primary-600"
              @click="store.markRead(item.id)"
            >
              {{ item.title }}
            </NuxtLink>
            <span v-else class="min-w-0 truncate text-[13px] font-medium text-[var(--text-strong)]">
              {{ item.title }}
            </span>
            <Button v-if="!item.read" variant="ghost" size="sm" @click="store.markRead(item.id)">
              标记已读
            </Button>
          </div>
          <p class="mt-0.5 text-[12px] leading-relaxed text-neutral-500">{{ item.body }}</p>
        </div>
      </article>
    </div>

    <div v-if="store.nextCursor" class="mt-4 text-center">
      <Button
        variant="secondary"
        size="sm"
        :loading="store.loading"
        @click="store.fetchList({ unreadOnly, append: true })"
      >
        加载更多
      </Button>
    </div>
  </div>
</template>
