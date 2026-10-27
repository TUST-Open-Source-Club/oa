<script setup lang="ts">
/** 门户主布局：侧栏导航 + 顶栏（未读角标、用户菜单）。 */
import { Button } from '@club-oa/ui'

const auth = useAuthStore()
const router = useRouter()
const notifications = useNotificationsStore()

const navItems = [
  { to: '/', label: '工作台', icon: 'home' },
  { to: '/im', label: '消息', icon: 'chat' },
  { to: '/tasks', label: '任务', icon: 'board' },
  { to: '/docs', label: '文档', icon: 'doc' },
  { to: '/events', label: '活动', icon: 'calendar' },
  { to: '/drive', label: '网盘', icon: 'folder' },
  { to: '/notifications', label: '通知', icon: 'bell', badgeKey: 'unread' },
  { to: '/admin/users', label: '用户管理', icon: 'users', adminOnly: true },
]

const mobileOpen = ref(false)

onMounted(() => {
  void notifications.fetchUnreadCount().catch(() => undefined)
})

/** 退出登录并回到登录页。 */
async function onLogout() {
  await auth.logout()
  await router.push('/login')
}
</script>

<template>
  <div class="flex min-h-screen bg-neutral-50 dark:bg-neutral-950">
    <!-- 侧栏 -->
    <aside
      class="fixed inset-y-0 left-0 z-40 w-60 shrink-0 border-r border-neutral-200 bg-white transition-transform md:static md:translate-x-0 dark:border-neutral-800 dark:bg-neutral-900"
      :class="mobileOpen ? 'translate-x-0' : '-translate-x-full'"
    >
      <div class="flex h-full flex-col px-3 py-4">
        <div class="mb-6 flex items-center gap-2.5 px-2">
          <span
            class="flex size-8 items-center justify-center rounded-lg bg-gradient-to-br from-primary-500 to-primary-700 text-sm font-bold text-white shadow-sm"
          >
            社
          </span>
          <span class="text-base font-semibold text-neutral-800 dark:text-neutral-100">
            {{ $config.public.appName }}
          </span>
        </div>
        <nav class="space-y-1">
          <NuxtLink
            v-for="item in navItems.filter((nav) => !nav.adminOnly || auth.isAdmin)"
            :key="item.to"
            :to="item.to"
            class="group flex items-center gap-3 rounded-[var(--radius-field)] px-3 py-2 text-sm text-neutral-600 transition hover:bg-neutral-100 dark:text-neutral-300 dark:hover:bg-neutral-800"
            active-class="bg-primary-50 font-medium text-primary-700 dark:bg-primary-900/40 dark:text-primary-200"
            @click="mobileOpen = false"
          >
            <AppIcon :name="item.icon" class="size-5 shrink-0 opacity-80" />
            <span class="flex-1">{{ item.label }}</span>
            <span
              v-if="item.badgeKey === 'unread' && notifications.unreadCount > 0"
              class="rounded-full bg-danger-500 px-1.5 py-0.5 text-[10px] leading-none text-white"
            >
              {{ notifications.unreadCount > 99 ? '99+' : notifications.unreadCount }}
            </span>
          </NuxtLink>
        </nav>
        <div class="mt-auto border-t border-neutral-200 pt-3 dark:border-neutral-800">
          <div class="flex items-center gap-2.5 px-2 py-2">
            <UserAvatar :name="auth.user?.nickname" :seed="auth.user?.id" />
            <div class="min-w-0 flex-1">
              <p class="truncate text-sm font-medium">{{ auth.user?.nickname }}</p>
              <p class="truncate text-xs text-neutral-400">{{ auth.user?.email }}</p>
            </div>
          </div>
          <div class="mt-1 flex gap-1 px-1">
            <NuxtLink
              to="/settings"
              class="flex flex-1 items-center justify-center gap-1.5 rounded-[var(--radius-field)] px-2 py-1.5 text-xs text-neutral-500 transition hover:bg-neutral-100 dark:hover:bg-neutral-800"
            >
              <AppIcon name="settings" class="size-4" /> 设置
            </NuxtLink>
            <Button variant="ghost" size="sm" class="flex-1 text-xs" @click="onLogout">
              <AppIcon name="logout" class="size-4" /> 退出
            </Button>
          </div>
        </div>
      </div>
    </aside>
    <div v-if="mobileOpen" class="fixed inset-0 z-30 bg-neutral-900/30 md:hidden" @click="mobileOpen = false" />

    <!-- 内容区 -->
    <div class="flex min-w-0 flex-1 flex-col">
      <header
        class="sticky top-0 z-20 flex h-14 items-center gap-3 border-b border-neutral-200 bg-white/80 px-4 backdrop-blur md:px-6 dark:border-neutral-800 dark:bg-neutral-900/80"
      >
        <button
          class="rounded p-1.5 text-neutral-500 hover:bg-neutral-100 md:hidden dark:hover:bg-neutral-800"
          aria-label="菜单"
          @click="mobileOpen = true"
        >
          <AppIcon name="menu" />
        </button>
        <div class="min-w-0 flex-1" />
        <NuxtLink
          to="/notifications"
          class="relative rounded-full p-2 text-neutral-500 transition hover:bg-neutral-100 dark:hover:bg-neutral-800"
          aria-label="通知"
        >
          <AppIcon name="bell" />
          <span
            v-if="notifications.unreadCount > 0"
            class="absolute -top-0.5 -right-0.5 flex min-w-4 items-center justify-center rounded-full bg-danger-500 px-1 text-[10px] leading-4 text-white"
          >
            {{ notifications.unreadCount > 99 ? '99+' : notifications.unreadCount }}
          </span>
        </NuxtLink>
        <UserAvatar :name="auth.user?.nickname" :seed="auth.user?.id" size="sm" />
      </header>
      <main class="flex-1 px-4 py-6 md:px-6">
        <slot />
      </main>
    </div>
  </div>
</template>
