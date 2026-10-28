<script setup lang="ts">
/**
 * 门户主布局：分组侧栏 + 顶栏（页面标题、通知、账号）。
 *
 * 视觉：Linear 风格——浅色侧栏、细边框、紧凑 32px 行高、选中项用「白底 + 细环」表达。
 */
const auth = useAuthStore()
const router = useRouter()
const route = useRoute()
const notifications = useNotificationsStore()

/** 主模块导航。 */
const primaryNav = [
  { to: '/', label: '工作台', icon: 'home' },
  { to: '/im', label: '消息', icon: 'chat' },
  { to: '/tasks', label: '任务', icon: 'board' },
  { to: '/docs', label: '文档', icon: 'doc' },
  { to: '/events', label: '活动', icon: 'calendar' },
  { to: '/drive', label: '网盘', icon: 'folder' },
]

/** 系统级导航。 */
const systemNav = [
  { to: '/notifications', label: '通知', icon: 'bell', badge: true, adminOnly: false },
  { to: '/settings', label: '设置', icon: 'settings', badge: false, adminOnly: false },
  { to: '/admin/users', label: '用户管理', icon: 'users', badge: false, adminOnly: true },
]

/** 路径 → 顶栏标题（按前缀匹配，长前缀优先）。 */
const titles: Array<[string, string]> = [
  ['/admin/users', '用户管理'],
  ['/admin', '管理'],
  ['/settings', '设置'],
  ['/notifications', '通知'],
  ['/drive', '网盘'],
  ['/events', '活动'],
  ['/docs', '文档'],
  ['/tasks', '任务'],
  ['/im', '消息'],
  ['/office', '在线编辑'],
  ['/install', '安装客户端'],
  ['/', '工作台'],
]

const pageTitle = computed(() => {
  const path = route.path
  const hit = titles.find(([prefix]) => (prefix === '/' ? path === '/' : path.startsWith(prefix)))
  return hit?.[1] ?? ''
})

const mobileOpen = ref(false)

/** 侧栏选中项样式：白底 + 极细环，靠层次而非色块。 */
const navActiveClass =
  'bg-[var(--surface)] text-[var(--text-strong)] shadow-[var(--shadow-card)] ring-1 ring-black/[0.04] dark:ring-white/[0.06]'

onMounted(() => {
  void notifications.fetchUnreadCount().catch(() => undefined)
})

watch(
  () => route.fullPath,
  () => {
    mobileOpen.value = false
  },
)

/** 退出登录并回到登录页。 */
async function onLogout() {
  await auth.logout()
  await router.push('/login')
}
</script>

<template>
  <div class="flex min-h-screen bg-[var(--canvas)]">
    <!-- 侧栏 -->
    <aside
      class="fixed inset-y-0 left-0 z-40 flex w-[232px] shrink-0 flex-col border-r border-[var(--border)] bg-[var(--surface-muted)] transition-transform md:static md:translate-x-0"
      :class="mobileOpen ? 'translate-x-0' : '-translate-x-full'"
    >
      <div class="flex h-12 items-center gap-2 px-3">
        <span
          class="flex size-6 items-center justify-center rounded-md bg-primary-600 text-[11px] font-semibold text-white"
        >
          社
        </span>
        <span class="truncate text-[13px] font-semibold text-[var(--text-strong)]">
          {{ $config.public.appName }}
        </span>
      </div>

      <nav class="mt-2 flex-1 space-y-6 overflow-y-auto px-2 pb-4">
        <div>
          <p class="px-2 pb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">
            工作区
          </p>
          <ul class="space-y-0.5">
            <li v-for="item in primaryNav" :key="item.to">
              <NuxtLink
                :to="item.to"
                class="group flex h-8 items-center gap-2 rounded-[var(--radius-field)] px-2 text-[13px] text-neutral-600 transition-colors hover:bg-neutral-200/60 dark:text-neutral-300 dark:hover:bg-neutral-800/60"
                :active-class="navActiveClass"
              >
                <AppIcon :name="item.icon" class="size-4 shrink-0 opacity-80" />
                <span class="truncate">{{ item.label }}</span>
              </NuxtLink>
            </li>
          </ul>
        </div>

        <div>
          <p class="px-2 pb-1.5 text-[10px] font-medium tracking-[0.12em] text-neutral-400 uppercase">
            系统
          </p>
          <ul class="space-y-0.5">
            <li v-for="item in systemNav.filter((nav) => !nav.adminOnly || auth.isAdmin)" :key="item.to">
              <NuxtLink
                :to="item.to"
                class="group flex h-8 items-center gap-2 rounded-[var(--radius-field)] px-2 text-[13px] text-neutral-600 transition-colors hover:bg-neutral-200/60 dark:text-neutral-300 dark:hover:bg-neutral-800/60"
                :active-class="navActiveClass"
              >
                <AppIcon :name="item.icon" class="size-4 shrink-0 opacity-80" />
                <span class="flex-1 truncate">{{ item.label }}</span>
                <span
                  v-if="item.badge && notifications.unreadCount > 0"
                  class="rounded-full bg-danger-500 px-1.5 text-[10px] leading-4 font-medium text-white"
                >
                  {{ notifications.unreadCount > 99 ? '99+' : notifications.unreadCount }}
                </span>
              </NuxtLink>
            </li>
          </ul>
        </div>
      </nav>

      <div class="border-t border-[var(--border)] p-2">
        <div class="flex items-center gap-2 rounded-[var(--radius-field)] px-1.5 py-1.5">
          <UserAvatar :name="auth.user?.nickname" :seed="auth.user?.id" size="sm" />
          <div class="min-w-0 flex-1">
            <p class="truncate text-[12px] font-medium text-[var(--text-strong)]">
              {{ auth.user?.nickname }}
            </p>
            <p class="truncate text-[11px] text-neutral-400">{{ auth.user?.email }}</p>
          </div>
          <button
            class="rounded-[var(--radius-field)] p-1 text-neutral-400 transition hover:bg-neutral-200/60 hover:text-neutral-600 dark:hover:bg-neutral-800/60"
            title="退出登录"
            aria-label="退出登录"
            @click="onLogout"
          >
            <AppIcon name="logout" class="size-4" />
          </button>
        </div>
      </div>
    </aside>

    <div
      v-if="mobileOpen"
      class="fixed inset-0 z-30 bg-neutral-950/30 md:hidden"
      @click="mobileOpen = false"
    />

    <!-- 内容区 -->
    <div class="flex min-w-0 flex-1 flex-col">
      <header
        class="sticky top-0 z-20 flex h-12 items-center gap-3 border-b border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_82%,transparent)] px-4 backdrop-blur-md md:px-6"
      >
        <button
          class="rounded-[var(--radius-field)] p-1 text-neutral-500 hover:bg-[var(--surface-hover)] md:hidden"
          aria-label="菜单"
          @click="mobileOpen = true"
        >
          <AppIcon name="menu" class="size-4" />
        </button>
        <h1 class="truncate text-[13px] font-medium text-[var(--text-strong)]">{{ pageTitle }}</h1>
        <div class="min-w-0 flex-1" />
        <NuxtLink
          to="/notifications"
          class="relative rounded-[var(--radius-field)] p-1.5 text-neutral-500 transition hover:bg-[var(--surface-hover)]"
          aria-label="通知"
        >
          <AppIcon name="bell" class="size-4" />
          <span
            v-if="notifications.unreadCount > 0"
            class="absolute -top-0.5 -right-0.5 flex min-w-4 items-center justify-center rounded-full bg-danger-500 px-1 text-[10px] leading-4 text-white"
          >
            {{ notifications.unreadCount > 99 ? '99+' : notifications.unreadCount }}
          </span>
        </NuxtLink>
        <NuxtLink to="/settings" aria-label="账号设置">
          <UserAvatar :name="auth.user?.nickname" :seed="auth.user?.id" size="sm" />
        </NuxtLink>
      </header>

      <main class="flex-1 px-4 py-6 md:px-6 lg:px-8">
        <slot />
      </main>
    </div>
  </div>
</template>
